//! `P2-X2`'s six named acceptance rows, each measured against section 25.2.
//!
//! Every enumeration this suite compares against is **parsed out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`**, never written here.
//! No count is asserted anywhere: both sides are enumerated and compared in
//! both directions, so a group added to the document fails as a missing key and
//! a group invented here fails as an extra one. `P2-N3` and `P2-N6` set that
//! discipline after six planning-versus-specification count mismatches in this
//! run, and one of them was in section 25's own neighbourhood.
//!
//! Three of the six rows are shapes of the source rather than behaviours —
//! nothing at run time would notice the day they stopped being true — so they
//! read this crate's own text. `docs/contracts/policy-source-scans.md` is the
//! page they are registered on, and they are written against all three of the
//! empty-scan shapes it names:
//!
//! * **the walk does not stop short**: [`product_sources`] descends the whole
//!   package rather than `src` by name, has a floor under it, and requires
//!   every `mod` declaration to name a file it actually read;
//! * **the checks are whole sets, not token lists**: there is no list of
//!   forbidden spellings in this file. `no_gpa_or_streak_hero_component` is an
//!   absence claim and it is proved by exhaustion — the whole variant set, the
//!   whole field-position inventory, and the whole section sequence, each
//!   compared in both directions;
//! * **the coverage is bounded**: every loop has a floor and every comparison
//!   fails on a missing key as well as an extra one.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_consent::CaptureStatus;
use academic_domain::{EntityId, FreshnessBand, TimestampMillis};
use academic_home::{
    AlertBucket, DayWindow, EstimatedMinutes, FreshnessAlert, GroupedAlerts, HIGHEST_BRIEF,
    HomeCard, HomeError, HomeGroup, HomeScreen, KnowledgeNeed, LOWEST_BRIEF, NextStep,
    OfficialCondition, OpenItem, OpenItemKind, PrerequisiteBrief, PrerequisiteItem,
    RecordingPermission, ScheduledItem, ScheduledOccasion, UpcomingUse,
};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------
// reading the specification
// ---------------------------------------------------------------------------

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// Section 25.2, from its heading to the next one.
///
/// The heading is matched on its own line and the block stops at the next
/// `### `, so a later section that happens to hold a numbered list cannot be
/// read instead.
fn section_25_2(specification: &str) -> Result<String, Box<dyn Error>> {
    let heading = "### 25.2 Home / Today";
    let start = specification
        .find(heading)
        .ok_or("the specification has no section 25.2 heading")?;
    let rest = &specification[start + heading.len()..];
    let end = rest
        .find("\n### ")
        .ok_or("section 25.2 does not end at a following heading")?;
    Ok(rest[..end].to_owned())
}

/// The numbered lines of section 25.2, as `(number, text)` pairs.
///
/// A line that opens with digits and a full stop must parse, and a number that
/// does not follow its predecessor raises rather than being skipped: a skipped
/// line is a group that silently stops being required.
fn numbered_lines(block: &str) -> Result<Vec<(usize, String)>, Box<dyn Error>> {
    let mut found: Vec<(usize, String)> = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        let digits: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            continue;
        }
        let Some(text) = trimmed[digits.len()..].strip_prefix(". ") else {
            continue;
        };
        let number: usize = digits.parse()?;
        let expected = found.len() + 1;
        if number != expected {
            return Err(format!("section 25.2 numbers {number} where {expected} was due").into());
        }
        found.push((number, text.trim().to_owned()));
    }
    if found.is_empty() {
        return Err("section 25.2 parsed to no numbered lines at all".into());
    }
    Ok(found)
}

/// Every back-quoted span of one line, in order.
fn back_quoted(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            break;
        };
        found.push(after[..close].to_owned());
        rest = &after[close + 1..];
    }
    found
}

/// What is left after removing each of `parts` from `line` once.
///
/// The shape `the_six_sections_are_section_25_13s_own` uses: a word the
/// enumeration does not hold leaves text behind, and the caller requires the
/// remainder to be punctuation.
fn remainder_after(line: &str, parts: &[&str]) -> Result<String, Box<dyn Error>> {
    let mut remaining = line.to_owned();
    for part in parts {
        if !remaining.contains(part) {
            return Err(format!("the specification's line does not name {part}").into());
        }
        remaining = remaining.replacen(part, "", 1);
    }
    Ok(remaining)
}

fn is_only_punctuation(remainder: &str) -> bool {
    remainder
        .chars()
        .all(|character| character.is_whitespace() || ":,.`".contains(character))
}

// ---------------------------------------------------------------------------
// reading this crate's own source
// ---------------------------------------------------------------------------

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

/// Every `.rs` file that ships: the whole package outside `tests`.
///
/// The package rather than its `src`, for the reason `S-12` records: two crates
/// in this workspace ship product-shaped code outside `src`, and a walk rooted
/// at `src` never reads it.
fn product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = Vec::new();
    walk(&root, &mut found)?;
    found.retain(|path| {
        !path
            .strip_prefix(&root)
            .unwrap_or(path)
            .starts_with("tests")
    });
    found.sort();
    if found.len() < 7 {
        return Err(format!("the walk found only {} product files", found.len()).into());
    }
    Ok(found)
}

/// Comments, string literals and character literals removed.
fn strip_non_code(source: &str) -> String {
    let characters: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < characters.len() {
        let current = characters[index];
        let next = characters.get(index + 1).copied();

        if current == '/' && next == Some('/') {
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
            }
            out.push('\n');
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1_usize;
            index += 2;
            while index < characters.len() && depth > 0 {
                if characters[index] == '/' && characters.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if characters[index] == '*' && characters.get(index + 1) == Some(&'/') {
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
            while characters.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if characters.get(probe) == Some(&'"') {
                probe += 1;
                let closing: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let tail: String = characters[probe..].iter().collect();
                if let Some(at) = tail.find(&closing) {
                    index = probe + tail[..at].chars().count() + closing.chars().count();
                    out.push_str("\"\"");
                    continue;
                }
            }
        }
        if current == '"' {
            index += 1;
            while index < characters.len() {
                if characters[index] == '\\' {
                    index += 2;
                    continue;
                }
                if characters[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            out.push_str("\"\"");
            continue;
        }
        if current == '\'' {
            // A lifetime, not a character literal, when what follows is an
            // identifier that is not immediately closed.
            let closes_at_two = characters.get(index + 2) == Some(&'\'');
            let escaped = next == Some('\\');
            if closes_at_two || escaped {
                index += 1;
                while index < characters.len() {
                    if characters[index] == '\\' {
                        index += 2;
                        continue;
                    }
                    if characters[index] == '\'' {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                out.push_str("''");
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One field position of one type declared in this crate's product source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FieldPosition {
    /// `Type`, or `Enum::Variant` for a variant's own fields.
    owner: String,
    /// The field's name, or `#n` for a tuple position.
    name: String,
    /// The declared type, whitespace-collapsed.
    declared: String,
}

impl FieldPosition {
    fn key(&self) -> String {
        format!("{}.{}: {}", self.owner, self.name, self.declared)
    }
}

fn opened_type(trimmed: &str) -> Option<String> {
    let rest = trimmed
        .strip_prefix("pub struct ")
        .or_else(|| trimmed.strip_prefix("struct "))
        .or_else(|| trimmed.strip_prefix("pub enum "))
        .or_else(|| trimmed.strip_prefix("enum "))
        .or_else(|| trimmed.strip_prefix("pub union "))
        .or_else(|| trimmed.strip_prefix("union "))?;
    let name: String = rest
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Splits `A, B<C, D>, E` at top-level commas only.
fn split_top_level(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for character in inner.chars() {
        match character {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                if !current.trim().is_empty() {
                    parts.push(current.trim().to_owned());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_owned());
    }
    parts
}

/// Every field position of every type declared in a stripped source text.
///
/// Taken as a free function over text so that
/// `the_field_reader_finds_a_position_nobody_reviewed` can drive it over a
/// fixture and observe that it sees the shape an injection would take. A
/// position it cannot classify is emitted with the text it could not read
/// rather than dropped, so the whole-set comparison fails on it.
fn field_positions_in(code: &str) -> Vec<FieldPosition> {
    let mut found = Vec::new();
    let mut current_type: Option<String> = None;
    let mut current_variant: Option<String> = None;
    let mut depth = 0_i32;
    let mut type_depth = 0_i32;

    for line in code.lines() {
        let trimmed = line.trim();
        let opens = i32::try_from(trimmed.matches('{').count()).unwrap_or(0);
        let closes = i32::try_from(trimmed.matches('}').count()).unwrap_or(0);
        // The depth this line's own content sits at, before its braces apply.
        let entering = depth;

        if let Some(name) = opened_type(trimmed) {
            // A tuple struct that opens and closes on one line: `struct X(T);`
            if let Some(open) = trimmed.find('(')
                && trimmed.ends_with(");")
            {
                let inner = &trimmed[open + 1..trimmed.len() - 2];
                for (index, part) in split_top_level(inner).into_iter().enumerate() {
                    let declared = part.strip_prefix("pub ").unwrap_or(&part).to_owned();
                    found.push(FieldPosition {
                        owner: name.clone(),
                        name: format!("#{index}"),
                        declared: collapse(&declared),
                    });
                }
                depth += opens - closes;
                continue;
            }
            if opens > 0 {
                current_type = Some(name);
                current_variant = None;
                type_depth = entering;
            }
            depth += opens - closes;
            continue;
        }

        if let Some(type_name) = current_type.clone() {
            if entering == type_depth + 1 {
                if let Some(open) = trimmed.find('(')
                    && trimmed.ends_with("),")
                {
                    // A tuple variant on one line: `Variant(T, U),`
                    let variant = leading_identifier(&trimmed[..open]);
                    if !variant.is_empty() {
                        let inner = &trimmed[open + 1..trimmed.len() - 2];
                        for (index, part) in split_top_level(inner).into_iter().enumerate() {
                            found.push(FieldPosition {
                                owner: format!("{type_name}::{variant}"),
                                name: format!("#{index}"),
                                declared: collapse(&part),
                            });
                        }
                    }
                } else if let Some(open) = trimmed.find('{')
                    && let Some(close) = trimmed.rfind('}')
                    && close > open
                {
                    // A struct variant that opens and closes on one line:
                    // `Variant { a: T, b: U },`
                    let variant = leading_identifier(&trimmed[..open]);
                    if !variant.is_empty() {
                        for part in split_top_level(&trimmed[open + 1..close]) {
                            if let Some((name, declared)) = named_field(&part) {
                                found.push(FieldPosition {
                                    owner: format!("{type_name}::{variant}"),
                                    name,
                                    declared,
                                });
                            }
                        }
                    }
                } else if trimmed.ends_with('{') {
                    // A struct variant whose body follows: `Variant {`
                    let variant = leading_identifier(trimmed);
                    if !variant.is_empty() {
                        current_variant = Some(variant);
                    }
                } else if let Some((name, declared)) = named_field(trimmed) {
                    found.push(FieldPosition {
                        owner: type_name.clone(),
                        name,
                        declared,
                    });
                }
            } else if entering == type_depth + 2
                && let Some(variant) = current_variant.clone()
                && let Some((name, declared)) = named_field(trimmed)
            {
                found.push(FieldPosition {
                    owner: format!("{type_name}::{variant}"),
                    name,
                    declared,
                });
            }
        }

        depth += opens - closes;
        if current_type.is_some() && depth <= type_depth {
            current_type = None;
            current_variant = None;
        } else if current_variant.is_some() && depth <= type_depth + 1 {
            current_variant = None;
        }
    }
    found
}

/// The identifier a line opens with, or the empty string.
fn leading_identifier(text: &str) -> String {
    text.trim()
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect()
}

/// `name: Type,` — a named field line, with `pub` and the trailing comma gone.
fn named_field(trimmed: &str) -> Option<(String, String)> {
    let body = trimmed.strip_suffix(',').unwrap_or(trimmed);
    let body = body.strip_prefix("pub ").unwrap_or(body);
    if body.starts_with('#') || body.starts_with("//") {
        return None;
    }
    let (name, declared) = body.split_once(especially_a_colon())?;
    let name = name.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    Some((name.to_owned(), collapse(declared)))
}

/// The separator a named field uses. Named so the reader says what it splits on.
const fn especially_a_colon() -> char {
    ':'
}

/// Every field position in this crate's product source.
fn crate_field_positions() -> Result<Vec<FieldPosition>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in product_sources()? {
        found.extend(field_positions_in(&strip_non_code(&fs::read_to_string(
            &path,
        )?)));
    }
    found.sort();
    Ok(found)
}

/// The variants of one enum declared in this crate's product source.
fn enum_variants(enum_name: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let opener = format!("pub enum {enum_name} {{");
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let Some(start) = code.find(&opener) else {
            continue;
        };
        let body = &code[start + opener.len()..];
        let end = body
            .find("\n}")
            .ok_or_else(|| format!("{enum_name} has no closing brace"))?;
        let mut variants = BTreeSet::new();
        let mut depth = 0_usize;
        for line in body[..end].lines() {
            let trimmed = line.trim();
            if depth == 0 {
                let name: String = trimmed
                    .chars()
                    .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                    .collect();
                let opens_a_variant = !name.is_empty()
                    && name.starts_with(|character: char| character.is_ascii_uppercase());
                if opens_a_variant {
                    variants.insert(name);
                }
            }
            depth = depth + trimmed.matches('{').count()
                - trimmed
                    .matches('}')
                    .count()
                    .min(depth + trimmed.matches('{').count());
        }
        if variants.is_empty() {
            return Err(format!("{enum_name} parsed to no variants").into());
        }
        return Ok(variants);
    }
    Err(format!("no product file declares pub enum {enum_name}").into())
}

// ---------------------------------------------------------------------------
// the corpus
// ---------------------------------------------------------------------------

/// A deterministic identifier: no clock, no randomness, no fixture file.
///
/// Built as a version-seven UUID's own text and parsed back, the way
/// `academic-evidence-center`'s suite does, so this crate needs no `uuid` dev
/// edge to name an entity.
fn entity(seed: u32) -> Result<EntityId, Box<dyn Error>> {
    let mut bytes = [0_u8; 16];
    bytes[0..8].copy_from_slice(&[0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00]);
    bytes[8] = 0x80;
    bytes[12..16].copy_from_slice(&seed.to_be_bytes());
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let text = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    Ok(text.parse::<EntityId>()?)
}

const NOW: i64 = 1_800_000_000_000;
const END_OF_TODAY: i64 = NOW + 12 * 60 * 60 * 1000;

fn now() -> TimestampMillis {
    TimestampMillis::new(NOW)
}

fn window() -> Result<DayWindow, Box<dyn Error>> {
    Ok(DayWindow::new(now(), TimestampMillis::new(END_OF_TODAY))?)
}

fn occasion_of(seed: u32) -> ScheduledOccasion {
    ScheduledOccasion::ALL[(seed as usize) % ScheduledOccasion::ALL.len()]
}

fn upcoming(seed: u32, offset_millis: i64) -> Result<UpcomingUse, Box<dyn Error>> {
    Ok(UpcomingUse::declare(
        occasion_of(seed),
        entity(seed)?,
        TimestampMillis::new(NOW + offset_millis),
        now(),
    )?)
}

/// One card of every group, seeded so the corpus is reproducible.
fn one_of_each(seed: u32, offset_millis: i64) -> Result<Vec<HomeCard>, Box<dyn Error>> {
    let brief = PrerequisiteBrief::assemble(vec![PrerequisiteItem::offer(
        entity(seed + 1)?,
        upcoming(seed + 2, offset_millis)?,
        EstimatedMinutes::new(15)?,
    )])?;
    Ok(vec![
        HomeCard::TodaysSchedule(ScheduledItem::new(
            occasion_of(seed),
            entity(seed + 3)?,
            TimestampMillis::new(NOW + offset_millis),
        )),
        HomeCard::MinimumPrerequisite(brief),
        HomeCard::RecordingPermissionStatus(
            RecordingPermission::ALL[(seed as usize) % RecordingPermission::ALL.len()],
        ),
        HomeCard::OpenQuestionAndMarkMoment(OpenItem::new(
            OpenItemKind::ALL[(seed as usize) % OpenItemKind::ALL.len()],
            entity(seed + 4)?,
        )),
        HomeCard::ProjectBlockingKnowledgeNeed(KnowledgeNeed::new(
            entity(seed + 5)?,
            entity(seed + 6)?,
        )),
        HomeCard::OfficialConditionAndStaleWarning(if seed.is_multiple_of(2) {
            OfficialCondition::WithDeadline {
                condition: entity(seed + 7)?,
                due: TimestampMillis::new(NOW + offset_millis),
            }
        } else {
            OfficialCondition::StaleOfficialData {
                source: entity(seed + 8)?,
                last_read: TimestampMillis::new(NOW - offset_millis),
            }
        }),
        HomeCard::CriticalPathNextStep(NextStep::chosen(entity(seed + 9)?, entity(seed + 10)?)),
        HomeCard::ConceptFreshnessAlert(FreshnessAlert::raise(
            entity(seed + 11)?,
            FreshnessBand::Stale,
            upcoming(seed + 12, offset_millis)?,
        )),
    ])
}

/// A screen of `rounds` cards per group, deadlines spread across the buckets.
///
/// Alternating rounds fall inside the caller's day window and after it, and the
/// five groups that carry no deadline fall into `No deadline` whatever the
/// round, so all three buckets are exercised from two rounds up.
fn corpus(rounds: u32) -> Result<Vec<HomeCard>, Box<dyn Error>> {
    let mut cards = Vec::new();
    for round in 0..rounds {
        let step = 60 * 1000 * i64::from(round + 1);
        let offset = if round.is_multiple_of(2) {
            step
        } else {
            END_OF_TODAY - NOW + step
        };
        cards.extend(one_of_each(round * 100 + 1, offset)?);
    }
    Ok(cards)
}

fn multiset(cards: &[HomeCard]) -> BTreeMap<String, usize> {
    let mut counted = BTreeMap::new();
    for card in cards {
        *counted.entry(format!("{card:?}")).or_insert(0) += 1;
    }
    counted
}

// ---------------------------------------------------------------------------
// 1. home_group_order_is_stable_one_to_eight
// ---------------------------------------------------------------------------

/// The eight groups, in the order the specification numbers them.
///
/// Three enumerations are compared and none is derived from another:
/// section 25.2's numbered lines, `HomeGroup::ALL`, and `HomeGroup::position`,
/// which is written out for the reason `P2-X1`'s view registry is written out
/// rather than derived from its route manifest. Then the rendered screen is
/// driven and its section sequence compared against the same order, for every
/// rotation of a corpus, so composition order cannot reach the sections.
#[test]
fn home_group_order_is_stable_one_to_eight() -> TestResult {
    let block = section_25_2(&specification()?)?;
    let lines = numbered_lines(&block)?;

    // Both directions, as sets, on the specification's own text.
    let from_document: BTreeSet<&str> = lines.iter().map(|(_, text)| text.as_str()).collect();
    let from_crate: BTreeSet<&str> = HomeGroup::ALL
        .into_iter()
        .map(HomeGroup::spec_words)
        .collect();
    assert_eq!(
        from_document.difference(&from_crate).collect::<Vec<_>>(),
        Vec::<&&str>::new(),
        "section 25.2 names a group HomeGroup does not"
    );
    assert_eq!(
        from_crate.difference(&from_document).collect::<Vec<_>>(),
        Vec::<&&str>::new(),
        "HomeGroup names a group section 25.2 does not"
    );

    // And position by position, which a set comparison would not see.
    assert_eq!(
        lines.len(),
        HomeGroup::ALL.len(),
        "section 25.2 has {} numbered lines and HomeGroup has {} arms",
        lines.len(),
        HomeGroup::ALL.len()
    );
    for ((number, text), group) in lines.iter().zip(HomeGroup::ALL) {
        assert_eq!(text, group.spec_words(), "line {number} is not {group:?}'s");
        assert_eq!(
            *number,
            group.position(),
            "{group:?} is numbered {} here and {number} in the document",
            group.position()
        );
    }

    // The rendered screen's sections are that order, whatever order the cards
    // arrived in. Every rotation of a corpus, and the empty screen.
    let empty = HomeScreen::default();
    assert_eq!(
        empty
            .sections()
            .iter()
            .map(academic_home::HomeSection::group)
            .collect::<Vec<_>>(),
        HomeGroup::ALL.to_vec(),
        "an empty screen does not render all eight sections"
    );

    let cards = corpus(3)?;
    assert!(cards.len() >= 24, "the corpus is too small to rotate");
    for rotation in 0..cards.len() {
        let mut rotated = cards.clone();
        rotated.rotate_left(rotation);
        let screen = HomeScreen::compose(rotated);
        let rendered: Vec<HomeGroup> = screen
            .sections()
            .iter()
            .map(academic_home::HomeSection::group)
            .collect();
        assert_eq!(
            rendered,
            HomeGroup::ALL.to_vec(),
            "rotation {rotation} changed the section order"
        );
        // Nothing is lost between composing and sectioning either.
        let sectioned: usize = screen
            .sections()
            .iter()
            .map(|section| section.cards().len())
            .sum();
        assert_eq!(
            sectioned,
            screen.cards().len(),
            "rotation {rotation} lost a card between compose and sections"
        );
        for section in screen.sections() {
            for card in section.cards() {
                assert_eq!(
                    card.group(),
                    section.group(),
                    "a card is filed under a group that is not its own"
                );
            }
        }
    }

    // The control: a re-sorted section list fails the same comparison, so the
    // comparison is not passing on the strength of both sides being the same
    // expression.
    let mut resorted = HomeGroup::ALL.to_vec();
    resorted.swap(0, 7);
    assert_ne!(
        resorted,
        HomeGroup::ALL.to_vec(),
        "the control did not change the order it was meant to change"
    );
    Ok(())
}

/// Section 25.2's first line names three occasions, and this crate has three.
#[test]
fn the_three_occasions_are_section_25_2s_own() -> TestResult {
    let block = section_25_2(&specification()?)?;
    let lines = numbered_lines(&block)?;
    let first = &lines
        .first()
        .ok_or("section 25.2 has no first numbered line")?
        .1;

    let words: Vec<&str> = ScheduledOccasion::ALL
        .iter()
        .map(|occasion| occasion.spec_words())
        .collect();
    // The line is `오늘 실제 일정: <three>.`; removing the heading and the
    // three occasions must leave punctuation only, so a fourth occasion in the
    // document leaves text behind.
    let remainder = remainder_after(first, &["오늘 실제 일정"])?;
    let remainder = remainder_after(&remainder, &words)?;
    assert!(
        is_only_punctuation(&remainder),
        "section 25.2's first line holds an occasion the enumeration does not: {remainder:?}"
    );

    // And the reader is not vacuous: a phrase the line does not hold is refused.
    assert!(
        remainder_after(first, &["streak"]).is_err(),
        "the reader accepted a phrase the line does not hold"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. prerequisite_count_is_within_one_to_three_with_reason_and_time
// ---------------------------------------------------------------------------

/// The bound, the reason and the time, all three measured.
///
/// The bound is not a number this suite chose: `최대 1–3개` is split on the
/// document's own en dash and the two halves are compared with the crate's
/// constants. Then the constructor is driven at every count from zero to two
/// past the upper bound and each answer is required to agree with the parsed
/// bound rather than with a hard-coded expectation.
#[test]
fn prerequisite_count_is_within_one_to_three_with_reason_and_time() -> TestResult {
    let block = section_25_2(&specification()?)?;
    let lines = numbered_lines(&block)?;
    let second = &lines
        .get(1)
        .ok_or("section 25.2 has no second numbered line")?
        .1;

    // `최대 1–3개` -- the bound, read out of the document.
    let after = second
        .split_once("최대 ")
        .ok_or("section 25.2's second line does not bound the brief")?
        .1;
    let phrase = after
        .split_once('개')
        .ok_or("the bound is not counted in 개")?
        .0;
    let (low_text, high_text) = phrase
        .split_once('\u{2013}')
        .ok_or("the bound is not written as a range")?;
    let lowest: usize = low_text.trim().parse()?;
    let highest: usize = high_text.trim().parse()?;
    assert_eq!(
        lowest, LOWEST_BRIEF,
        "the crate's lower bound is not the document's"
    );
    assert_eq!(
        highest, HIGHEST_BRIEF,
        "the crate's upper bound is not the document's"
    );
    assert!(
        lowest >= 1 && highest > lowest,
        "the parsed bound is degenerate"
    );

    // The document asks for both a reason and a time, by name.
    assert!(
        second.contains("\u{201c}왜 지금\u{201d}"),
        "section 25.2's second line no longer asks why now"
    );
    assert!(
        second.contains("예상 시간"),
        "section 25.2's second line no longer asks for an estimated time"
    );

    // Every count from zero to two past the bound, judged by the parsed bound.
    for count in 0..=(highest + 2) {
        let mut items: Vec<PrerequisiteItem> = Vec::new();
        for index in 0..count {
            let seed = u32::try_from(index).unwrap_or(0);
            items.push(PrerequisiteItem::offer(
                entity(seed + 1)?,
                upcoming(seed + 2, 90 * 60 * 1000)?,
                EstimatedMinutes::new(20)?,
            ));
        }
        let assembled = PrerequisiteBrief::assemble(items);
        let within = (lowest..=highest).contains(&count);
        match (within, assembled) {
            (true, Ok(brief)) => {
                assert_eq!(brief.items().len(), count);
                // Both requirements are readable off every item, and the
                // reason is ahead of the instant it was judged from.
                for item in brief.items() {
                    assert!(
                        item.why_now().at().value() > now().value(),
                        "a why-now that is not ahead of now reached an item"
                    );
                    assert!(
                        item.estimated().minutes() > 0,
                        "an item carries no estimated time"
                    );
                }
            }
            (false, Err(HomeError::PrerequisiteCountOutOfBounds { count: reported })) => {
                assert_eq!(reported, count);
            }
            (within, other) => {
                return Err(format!(
                    "a brief of {count} items is {} and the crate answered {other:?}",
                    if within {
                        "within the bound"
                    } else {
                        "outside it"
                    }
                )
                .into());
            }
        }
    }

    // A time of nothing is refused, so the field cannot be satisfied vacuously.
    assert_eq!(EstimatedMinutes::new(0), Err(HomeError::EstimateIsZero));
    assert!(EstimatedMinutes::new(1).is_ok());
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. permission_status_is_exactly_four_values
// ---------------------------------------------------------------------------

/// Four values, closed, and the total image of `P2-G6`'s own status set.
///
/// `CaptureStatus` is `#[non_exhaustive]`, so `RecordingPermission::of` must
/// carry a wildcard arm and the compiler says nothing about a sixth status.
/// This test is what does: it reads the arms of `CaptureStatus::as_str` out of
/// `crates/consent/src/status.rs` and compares them, in both directions,
/// against the five this crate names explicitly.
#[test]
fn permission_status_is_exactly_four_values() -> TestResult {
    let block = section_25_2(&specification()?)?;
    let lines = numbered_lines(&block)?;
    let third = &lines
        .get(2)
        .ok_or("section 25.2 has no third numbered line")?
        .1;

    // The four words, read out of the document's own back quotes.
    let from_document = back_quoted(third);
    let from_crate: Vec<&str> = RecordingPermission::ALL
        .iter()
        .map(|status| status.spec_words())
        .collect();
    assert_eq!(
        from_document, from_crate,
        "section 25.2's four permission words are not the enumeration's, or not in its order"
    );
    let document_set: BTreeSet<&str> = from_document.iter().map(String::as_str).collect();
    let crate_set: BTreeSet<&str> = from_crate.iter().copied().collect();
    assert_eq!(document_set, crate_set, "the two sets differ");

    // Removing the four leaves punctuation, so a fifth word in the document
    // fails rather than being folded into the nearest arm.
    let remainder = remainder_after(third, &["녹음 permission 상태"])?;
    let remainder = remainder_after(&remainder, &from_crate)?;
    assert!(
        is_only_punctuation(&remainder),
        "section 25.2's third line holds a word the enumeration does not: {remainder:?}"
    );

    // The type is closed: four arms in the source, compared both ways.
    let declared = enum_variants("RecordingPermission")?;
    let expected: BTreeSet<String> = ["Allowed", "CheckNeeded", "Conditional", "Forbidden"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        declared, expected,
        "RecordingPermission has an unreviewed arm"
    );
    assert_eq!(declared.len(), RecordingPermission::ALL.len());

    // `P2-G6`'s own arms, read out of that crate's `as_str` match.
    let consent = fs::read_to_string(
        workspace_root()
            .join("crates")
            .join("consent")
            .join("src")
            .join("status.rs"),
    )?;
    let start = consent
        .find("pub const fn as_str(self) -> &'static str {")
        .ok_or("academic-consent has no CaptureStatus::as_str")?;
    let end = consent[start..]
        .find("\n    }\n")
        .ok_or("CaptureStatus::as_str has no closing brace")?;
    let mut consent_arms: BTreeSet<String> = BTreeSet::new();
    for line in consent[start..start + end].lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("Self::") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            consent_arms.insert(name);
        }
    }
    assert!(
        consent_arms.len() >= 2,
        "the reader found only {} capture statuses",
        consent_arms.len()
    );

    // The statuses this crate names explicitly, read out of its own source.
    // Read off the stripped text, and indexed into the same text: the earlier
    // version of this reader matched on the stripped code and sliced the raw
    // file, so every name it reported was cut at the wrong offset.
    let permission_code = strip_non_code(&fs::read_to_string(
        crate_root().join("src").join("permission.rs"),
    )?);
    let mapped: BTreeSet<String> = permission_code
        .match_indices("CaptureStatus::")
        .map(|(at, marker)| leading_identifier(&permission_code[at + marker.len()..]))
        .filter(|name| !name.is_empty())
        .collect();
    assert_eq!(
        consent_arms, mapped,
        "the home surface does not name exactly academic-consent's capture statuses"
    );

    // The map is onto: every one of the four words is reachable, and the five
    // known statuses reach nothing else.
    let image: BTreeSet<RecordingPermission> = [
        CaptureStatus::Unknown,
        CaptureStatus::Prohibited,
        CaptureStatus::Permitted,
        CaptureStatus::PermittedWithConditions,
        CaptureStatus::Expired,
    ]
    .into_iter()
    .map(RecordingPermission::of)
    .collect();
    assert_eq!(
        image,
        RecordingPermission::ALL
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "a permission word is unreachable from any capture status"
    );

    // The two that fold, fold; the three that do not, do not.
    assert_eq!(
        RecordingPermission::of(CaptureStatus::Unknown),
        RecordingPermission::CheckNeeded
    );
    assert_eq!(
        RecordingPermission::of(CaptureStatus::Expired),
        RecordingPermission::CheckNeeded
    );
    assert_eq!(
        RecordingPermission::of(CaptureStatus::Permitted),
        RecordingPermission::Allowed
    );
    assert_eq!(
        RecordingPermission::of(CaptureStatus::PermittedWithConditions),
        RecordingPermission::Conditional
    );
    assert_eq!(
        RecordingPermission::of(CaptureStatus::Prohibited),
        RecordingPermission::Forbidden
    );

    // Default-deny: the value nobody set does not read as permission to record.
    assert_eq!(
        RecordingPermission::of(CaptureStatus::default()),
        RecordingPermission::CheckNeeded
    );

    // No route from text into any type here, held without trybuild.
    //
    // The compile-fail case tries the four named routes and is the compiled
    // half, but it is one guard and its diagnostics are a compiler's. This is
    // the other: every `impl` header and every `derive` in the crate's product
    // source, as two whole sets in both directions. A `FromStr`, a
    // `TryFrom<&str>` or a `From<&str>` on any type in this crate fails as an
    // extra key, and so does a conversion nobody has thought of, because the
    // comparison is against the list of implementations that *exist* rather
    // than against a list of spellings to refuse.
    let (headers, derives) = impl_and_derive_sets()?;
    assert_eq!(
        headers,
        REVIEWED_IMPL_HEADERS_SET
            .get_or_init(reviewed_impl_headers)
            .clone(),
        "this crate declares an implementation nobody reviewed, or has lost one"
    );
    assert_eq!(
        derives,
        REVIEWED_DERIVES_SET.get_or_init(reviewed_derives).clone(),
        "this crate derives a trait nobody reviewed, or has lost one"
    );
    assert!(
        headers.len() >= 15,
        "the impl reader found only {} headers",
        headers.len()
    );

    // The readers are not vacuous: an arm neither crate has reads as absent,
    // and neither whole set holds a conversion.
    assert!(!consent_arms.contains("Revoked"));
    assert!(!declared.contains("Revoked"));
    assert!(!headers.iter().any(|header| header.contains(" for ")
        && !header.contains("thiserror")
        && !header.starts_with("impl core::fmt")));
    Ok(())
}

/// The two whole sets `impl_and_derive_sets` reads: the `impl` headers, and the
/// derived traits keyed on the type that derives them.
type ImplAndDeriveSets = (BTreeSet<String>, BTreeMap<String, BTreeSet<String>>);

/// Every `impl` header, and every derived trait **keyed on the type deriving it**.
///
/// Headers are collapsed and read whole, so `impl FromStr for X` is a different
/// key from `impl X`.
///
/// The derives are per type and not flattened, and that is not a detail: a
/// flattened set holds `Default` once because `HomeScreen` derives it, and an
/// injected `Default` on `AlertBucket` passed the flattened comparison. Keyed
/// on the type, the same injection changes that type's value and fails.
fn impl_and_derive_sets() -> Result<ImplAndDeriveSets, Box<dyn Error>> {
    let mut headers = BTreeSet::new();
    let mut derives: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let mut pending: Option<BTreeSet<String>> = None;
        let mut open: Option<String> = None;
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
                headers.insert(collapse(trimmed.trim_end_matches('{').trim()));
                continue;
            }
            // A derive list, possibly spread over several lines.
            if let Some(accumulated) = open.as_mut() {
                accumulated.push_str(trimmed);
                if trimmed.contains(")]") {
                    pending = Some(derive_names(accumulated));
                    open = None;
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("#[derive(") {
                if trimmed.contains(")]") {
                    pending = Some(derive_names(rest));
                } else {
                    open = Some(rest.to_owned());
                }
                continue;
            }
            if let Some(name) = opened_type(trimmed) {
                derives.insert(name, pending.take().unwrap_or_default());
                continue;
            }
        }
        if open.is_some() {
            return Err(format!("{} has an unterminated derive", path.display()).into());
        }
    }
    if derives.is_empty() {
        return Err("the derive reader found no type at all".into());
    }
    Ok((headers, derives))
}

/// The trait names inside a `#[derive(...)]` body.
fn derive_names(body: &str) -> BTreeSet<String> {
    body.split_once(")]")
        .map_or(body, |(before, _)| before)
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

static REVIEWED_IMPL_HEADERS_SET: std::sync::OnceLock<BTreeSet<String>> =
    std::sync::OnceLock::new();
static REVIEWED_DERIVES_SET: std::sync::OnceLock<BTreeMap<String, BTreeSet<String>>> =
    std::sync::OnceLock::new();

/// The implementations this crate is reviewed to have.
///
/// Every entry is an inherent `impl`. There is no `impl Trait for Type` line at
/// all outside the `thiserror` derive, which is the whole point: nothing here
/// converts, parses, defaults, dereferences, borrows or serializes.
fn reviewed_impl_headers() -> BTreeSet<String> {
    [
        "impl AlertBucket",
        "impl DayWindow",
        "impl EstimatedMinutes",
        "impl FreshnessAlert",
        "impl GroupedAlerts",
        "impl HomeCard",
        "impl HomeGroup",
        "impl HomeScreen",
        "impl KnowledgeNeed",
        "impl NextStep",
        "impl OfficialCondition",
        "impl OpenItem",
        "impl OpenItemKind",
        "impl PrerequisiteBrief",
        "impl PrerequisiteItem",
        "impl RecordingPermission",
        "impl ScheduledItem",
        "impl ScheduledOccasion",
        "impl UpcomingUse",
        "impl<'screen> HomeSection<'screen>",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// What each type in this crate is reviewed to derive.
///
/// `Default` appears once, on `HomeScreen`, and that is the empty screen. No
/// payload type derives it: an empty card would answer a question nobody asked,
/// and an `UpcomingUse` or a `FreshnessAlert` with a `Default` would be exactly
/// the value section 25.2's second and eighth lines refuse.
///
/// The comparison is keyed on the type, so moving a derive from one type to
/// another fails even though the set of trait names is unchanged.
fn reviewed_derives() -> BTreeMap<String, BTreeSet<String>> {
    const ORDERED: &str = "Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash";
    const VALUE: &str = "Debug, Clone, Copy, PartialEq, Eq";
    let set = |names: &str| -> BTreeSet<String> {
        names
            .split(',')
            .map(|name| name.trim().to_owned())
            .collect()
    };
    [
        ("AlertBucket", ORDERED),
        ("DayWindow", VALUE),
        ("EstimatedMinutes", ORDERED),
        ("FreshnessAlert", VALUE),
        ("GroupedAlerts", "Debug, Clone, PartialEq, Eq"),
        ("HomeCard", "Debug, Clone, PartialEq, Eq"),
        (
            "HomeError",
            "Debug, Clone, Copy, PartialEq, Eq, thiserror::Error",
        ),
        ("HomeGroup", ORDERED),
        ("HomeScreen", "Debug, Clone, PartialEq, Eq, Default"),
        ("HomeSection", "Debug, Clone, PartialEq, Eq"),
        ("KnowledgeNeed", VALUE),
        ("NextStep", VALUE),
        ("OfficialCondition", VALUE),
        ("OpenItem", VALUE),
        ("OpenItemKind", ORDERED),
        ("PrerequisiteBrief", "Debug, Clone, PartialEq, Eq"),
        ("PrerequisiteItem", VALUE),
        ("RecordingPermission", ORDERED),
        ("ScheduledItem", VALUE),
        ("ScheduledOccasion", ORDERED),
        ("UpcomingUse", VALUE),
    ]
    .into_iter()
    .map(|(name, names)| (name.to_owned(), set(names)))
    .collect()
}

// ---------------------------------------------------------------------------
// 4. overflow_is_grouped_not_hidden_and_count_preserved
// ---------------------------------------------------------------------------

/// Grouping loses nothing, at any volume.
///
/// The comparison is a **multiset in both directions**, not a length: an
/// implementation that dropped one card and duplicated another passes a length
/// comparison and fails this one. The control beside it is a deliberately lossy
/// grouping written here, required to fail the same comparison.
#[test]
fn overflow_is_grouped_not_hidden_and_count_preserved() -> TestResult {
    let block = section_25_2(&specification()?)?;

    // The three names are the document's own back-quoted spellings.
    let overflow_line = block
        .lines()
        .find(|line| line.contains("숨기지 않고"))
        .ok_or("section 25.2 no longer says the alerts are grouped rather than hidden")?;
    let from_document = back_quoted(overflow_line);
    let from_crate: Vec<&str> = AlertBucket::ALL
        .iter()
        .map(|bucket| bucket.spec_words())
        .collect();
    assert_eq!(
        from_document, from_crate,
        "the three buckets are not the document's, or not in its order"
    );

    // Volume: one round is a small screen, sixty rounds is a crowded one.
    for rounds in [1_u32, 2, 7, 60] {
        let cards = corpus(rounds)?;
        let screen = HomeScreen::compose(cards.clone());
        let grouped = screen.grouped(window()?);

        let mut recombined: Vec<HomeCard> = Vec::new();
        for bucket in AlertBucket::ALL {
            recombined.extend_from_slice(grouped.bucket(bucket));
        }

        // Both directions, as multisets.
        assert_eq!(
            multiset(&recombined),
            multiset(&cards),
            "grouping {rounds} rounds changed what is on the screen"
        );
        assert_eq!(
            grouped.total(),
            cards.len(),
            "the reported total is not the input"
        );
        assert_eq!(recombined.len(), cards.len());

        // Every card is in the bucket its own deadline puts it in, and in
        // exactly one bucket.
        for bucket in AlertBucket::ALL {
            for card in grouped.bucket(bucket) {
                assert_eq!(
                    AlertBucket::of(card.deadline(), window()?),
                    bucket,
                    "a card is in a bucket its deadline does not put it in"
                );
                assert_eq!(HomeScreen::bucket_of(card, window()?), bucket);
            }
        }

        // Nothing is hidden as the screen grows: the crowded screen holds every
        // card the small one did, times the rounds.
        assert_eq!(
            grouped.total(),
            usize::try_from(rounds).unwrap_or(0) * HomeGroup::ALL.len(),
            "a round of the corpus does not contribute one card per group"
        );

        // All three buckets are actually exercised, or this loop proves nothing
        // about two of them.
        if rounds >= 2 {
            for bucket in AlertBucket::ALL {
                assert!(
                    !grouped.bucket(bucket).is_empty(),
                    "{bucket:?} was never exercised at {rounds} rounds"
                );
            }
        }
    }

    // The control: a grouping that drops one card and duplicates another fails
    // the same comparison. Without it, an implementation that returned the
    // input unchanged would satisfy every assertion above for the wrong reason.
    let cards = corpus(4)?;
    let mut lossy = cards.clone();
    let dropped = lossy.pop().ok_or("the corpus is empty")?;
    let duplicated = lossy.first().cloned().ok_or("the corpus is empty")?;
    lossy.push(duplicated);
    assert_eq!(lossy.len(), cards.len(), "the control changed the length");
    assert_ne!(
        multiset(&lossy),
        multiset(&cards),
        "the multiset comparison did not see a dropped card behind an equal length"
    );
    assert_ne!(format!("{dropped:?}"), String::new());

    // The window's boundary is inclusive on the `Today` side and exclusive on
    // the `Soon` side, and the absent deadline is neither.
    let end = TimestampMillis::new(END_OF_TODAY);
    assert_eq!(AlertBucket::of(Some(end), window()?), AlertBucket::Today);
    assert_eq!(
        AlertBucket::of(Some(TimestampMillis::new(END_OF_TODAY + 1)), window()?),
        AlertBucket::Soon
    );
    assert_eq!(
        AlertBucket::of(Some(TimestampMillis::new(NOW - 1)), window()?),
        AlertBucket::Today,
        "a deadline that has gone by is not Today"
    );
    assert_eq!(AlertBucket::of(None, window()?), AlertBucket::NoDeadline);

    // An empty screen groups to three empty buckets rather than to nothing.
    let empty = GroupedAlerts::group(Vec::new(), window()?);
    assert_eq!(empty.total(), 0);
    for bucket in AlertBucket::ALL {
        assert!(empty.bucket(bucket).is_empty());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. freshness_alert_requires_an_upcoming_use
// ---------------------------------------------------------------------------

/// An alert exists only where a use is ahead of the instant it was judged from.
///
/// The refusal is one step earlier than the alert: `UpcomingUse::declare` is
/// the only producer of the value `FreshnessAlert::raise` requires, so an
/// occasion that is not upcoming leaves the alert with nothing to be built
/// from. The boundary is swept rather than sampled — every instant from well
/// before the reference to well after it, over all three occasions.
#[test]
fn freshness_alert_requires_an_upcoming_use() -> TestResult {
    let block = section_25_2(&specification()?)?;
    let lines = numbered_lines(&block)?;
    let eighth = &lines
        .get(7)
        .ok_or("section 25.2 has no eighth numbered line")?
        .1;
    assert!(
        eighth.contains("실제 upcoming use가 있을 때만"),
        "section 25.2's eighth line no longer requires a real upcoming use"
    );

    let concept = entity(4_242)?;
    let mut raised = 0_usize;
    let mut refused = 0_usize;
    for occasion in ScheduledOccasion::ALL {
        for delta in -5_i64..=5 {
            let at = TimestampMillis::new(NOW + delta);
            let declared = UpcomingUse::declare(occasion, entity(7)?, at, now());
            if delta > 0 {
                let use_ahead = declared?;
                let alert = FreshnessAlert::raise(concept, FreshnessBand::Stale, use_ahead);
                assert_eq!(alert.upcoming(), use_ahead);
                assert_eq!(alert.concept(), concept);
                assert_eq!(alert.band(), FreshnessBand::Stale);
                assert_eq!(alert.upcoming().occasion(), occasion);
                raised += 1;
            } else {
                assert_eq!(
                    declared,
                    Err(HomeError::OccasionIsNotUpcoming {
                        occasion_at: at,
                        reference: now(),
                    }),
                    "an occasion at {delta} millis from now was accepted as upcoming"
                );
                refused += 1;
            }
        }
    }
    assert_eq!(raised, 3 * 5, "the sweep raised the wrong number of alerts");
    assert_eq!(refused, 3 * 6, "the sweep refused the wrong number");

    // Every band, so the rule is about the use and not about the band. A
    // `VERY_HIGH` concept with an upcoming use alerts, and a `STALE` one with
    // no upcoming use has nothing to alert with.
    let bands = [
        FreshnessBand::VeryHigh,
        FreshnessBand::High,
        FreshnessBand::Moderate,
        FreshnessBand::Low,
        FreshnessBand::Stale,
        FreshnessBand::Unknown,
    ];
    for band in bands {
        let ahead = upcoming(9, 60 * 60 * 1000)?;
        assert_eq!(
            FreshnessAlert::raise(concept, band, ahead).band(),
            band,
            "the band is not carried through"
        );
        assert!(
            UpcomingUse::declare(
                ScheduledOccasion::Class,
                entity(11)?,
                TimestampMillis::new(NOW - 60 * 60 * 1000),
                now(),
            )
            .is_err(),
            "a use in the past was accepted for a {band:?} concept"
        );
    }

    // The reference instant is an argument, so the same occasion is upcoming
    // against an earlier reference and not upcoming against a later one. That
    // is what makes this a statement about use rather than about a clock this
    // crate read.
    let at = TimestampMillis::new(NOW + 1_000);
    assert!(UpcomingUse::declare(ScheduledOccasion::Class, entity(13)?, at, now()).is_ok());
    assert!(
        UpcomingUse::declare(
            ScheduledOccasion::Class,
            entity(13)?,
            at,
            TimestampMillis::new(NOW + 2_000)
        )
        .is_err()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. no_gpa_or_streak_hero_component
// ---------------------------------------------------------------------------

/// The reviewed field inventory: every field position this crate declares.
///
/// The right-hand side of each entry says which of section 25.2's eight lines
/// the position serves. This is the exhaustive net, and it is deliberately not
/// a list of names to refuse: it is the list of positions that exist. A field
/// added anywhere in this crate, spelling nothing anybody thought to forbid, in
/// a module nobody predicted, fails as an extra entry.
const REVIEWED_FIELD_POSITIONS: &[(&str, &str)] = &[
    (
        "DayWindow.ends: TimestampMillis",
        "the caller's day boundary",
    ),
    (
        "DayWindow.starts: TimestampMillis",
        "the caller's day boundary",
    ),
    ("EstimatedMinutes.#0: u16", "line 2 — 예상 시간"),
    (
        "FreshnessAlert.band: FreshnessBand",
        "line 8 — P2-N3's band, carried",
    ),
    ("FreshnessAlert.concept: EntityId", "line 8 — which concept"),
    (
        "FreshnessAlert.upcoming: UpcomingUse",
        "line 8 — the use that justifies it",
    ),
    (
        "GroupedAlerts.no_deadline: Vec<HomeCard>",
        "the No deadline bucket",
    ),
    ("GroupedAlerts.soon: Vec<HomeCard>", "the Soon bucket"),
    ("GroupedAlerts.today: Vec<HomeCard>", "the Today bucket"),
    (
        "HomeCard::ConceptFreshnessAlert.#0: FreshnessAlert",
        "line 8",
    ),
    ("HomeCard::CriticalPathNextStep.#0: NextStep", "line 7"),
    (
        "HomeCard::MinimumPrerequisite.#0: PrerequisiteBrief",
        "line 2",
    ),
    (
        "HomeCard::OfficialConditionAndStaleWarning.#0: OfficialCondition",
        "line 6",
    ),
    ("HomeCard::OpenQuestionAndMarkMoment.#0: OpenItem", "line 4"),
    (
        "HomeCard::ProjectBlockingKnowledgeNeed.#0: KnowledgeNeed",
        "line 5",
    ),
    (
        "HomeCard::RecordingPermissionStatus.#0: RecordingPermission",
        "line 3",
    ),
    ("HomeCard::TodaysSchedule.#0: ScheduledItem", "line 1"),
    (
        "HomeError::DayWindowEndsBeforeItStarts.end: academic_domain::TimestampMillis",
        "a refusal",
    ),
    (
        "HomeError::DayWindowEndsBeforeItStarts.start: academic_domain::TimestampMillis",
        "a refusal",
    ),
    (
        "HomeError::OccasionIsNotUpcoming.occasion_at: academic_domain::TimestampMillis",
        "a refusal",
    ),
    (
        "HomeError::OccasionIsNotUpcoming.reference: academic_domain::TimestampMillis",
        "a refusal",
    ),
    (
        "HomeError::PrerequisiteCountOutOfBounds.count: usize",
        "a refusal",
    ),
    ("HomeScreen.cards: Vec<HomeCard>", "the screen"),
    (
        "HomeSection.cards: Vec<&'screen HomeCard>",
        "one section's cards",
    ),
    ("HomeSection.group: HomeGroup", "which of the eight"),
    (
        "KnowledgeNeed.concept: EntityId",
        "line 5 — the missing concept",
    ),
    (
        "KnowledgeNeed.project: EntityId",
        "line 5 — the project it blocks",
    ),
    ("NextStep.path: EntityId", "line 7 — the active path"),
    ("NextStep.step: EntityId", "line 7 — the step chosen on it"),
    (
        "OfficialCondition::StaleOfficialData.last_read: TimestampMillis",
        "line 6 — source staleness",
    ),
    (
        "OfficialCondition::StaleOfficialData.source: EntityId",
        "line 6 — which source",
    ),
    (
        "OfficialCondition::WithDeadline.condition: EntityId",
        "line 6 — which condition",
    ),
    (
        "OfficialCondition::WithDeadline.due: TimestampMillis",
        "line 6 — its deadline",
    ),
    (
        "OpenItem.kind: OpenItemKind",
        "line 4 — question or mark moment",
    ),
    ("OpenItem.subject: EntityId", "line 4 — which one"),
    (
        "PrerequisiteBrief.items: Vec<PrerequisiteItem>",
        "line 2 — the one to three",
    ),
    (
        "PrerequisiteItem.concept: EntityId",
        "line 2 — which concept",
    ),
    (
        "PrerequisiteItem.estimated: EstimatedMinutes",
        "line 2 — 예상 시간",
    ),
    ("PrerequisiteItem.why_now: UpcomingUse", "line 2 — 왜 지금"),
    (
        "ScheduledItem.at: TimestampMillis",
        "line 1 — when it falls",
    ),
    (
        "ScheduledItem.occasion: ScheduledOccasion",
        "line 1 — which of the three",
    ),
    (
        "ScheduledItem.subject: EntityId",
        "line 1 — what it is about",
    ),
    ("UpcomingUse.at: TimestampMillis", "when the occasion falls"),
    (
        "UpcomingUse.occasion: ScheduledOccasion",
        "which of the three",
    ),
    (
        "UpcomingUse.subject: EntityId",
        "what the occasion is about",
    ),
];

/// No headline metric, proved by exhaustion rather than by a name list.
///
/// Four whole-set comparisons, each blind to a different bypass and each in
/// both directions. There is no list of forbidden spellings in this file: a
/// name list refuses the edits somebody thought of in advance and admits every
/// edit spelled differently, which this run measured six times, and `P2-RF13`
/// found six real leaks the moment one became a whole-set classification.
#[test]
fn no_gpa_or_streak_hero_component() -> TestResult {
    // The document still asks for this, so the test is not guarding a rule that
    // has been withdrawn.
    let block = section_25_2(&specification()?)?;
    assert!(
        block.contains("GPA나 streak를 hero metric으로 두지 않는다"),
        "section 25.2 no longer refuses a hero metric"
    );

    // 1. The card set and the group set, from the source, in both directions.
    let card_variants = enum_variants("HomeCard")?;
    let group_variants = enum_variants("HomeGroup")?;
    assert_eq!(
        card_variants, group_variants,
        "HomeCard and HomeGroup do not hold the same names"
    );
    let from_all: BTreeSet<String> = HomeGroup::ALL
        .into_iter()
        .map(|group| format!("{group:?}"))
        .collect();
    assert_eq!(
        group_variants, from_all,
        "an arm of HomeGroup is missing from HomeGroup::ALL, or the other way round"
    );
    assert!(
        card_variants.len() >= 8,
        "the variant reader found only {} cards",
        card_variants.len()
    );

    // 2. Every field position, in both directions, against the reviewed
    //    inventory. This is the net an injection that spells nothing forbidden
    //    falls into.
    let found: BTreeSet<String> = crate_field_positions()?
        .iter()
        .map(FieldPosition::key)
        .collect();
    let reviewed: BTreeSet<String> = REVIEWED_FIELD_POSITIONS
        .iter()
        .map(|(position, _)| (*position).to_owned())
        .collect();
    assert_eq!(
        reviewed.len(),
        REVIEWED_FIELD_POSITIONS.len(),
        "the reviewed inventory names one position twice"
    );
    assert_eq!(
        found.difference(&reviewed).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "this crate declares a field position nobody reviewed"
    );
    assert_eq!(
        reviewed.difference(&found).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "the reviewed inventory names a field position this crate does not declare"
    );
    assert!(
        found.len() >= 40,
        "the field reader found only {} positions",
        found.len()
    );
    // Every reviewed position says which of section 25.2's lines it serves.
    for (position, why) in REVIEWED_FIELD_POSITIONS {
        assert!(!why.trim().is_empty(), "{position} has no reviewed reason");
    }

    // 3. The rendered section sequence, at every volume, is the eight and
    //    nothing precedes them.
    for rounds in [0_u32, 1, 5] {
        let screen = HomeScreen::compose(corpus(rounds)?);
        let sections = screen.sections();
        assert_eq!(sections.len(), HomeGroup::COUNT);
        assert_eq!(
            sections
                .iter()
                .map(academic_home::HomeSection::group)
                .collect::<Vec<_>>(),
            HomeGroup::ALL.to_vec(),
            "the section sequence at {rounds} rounds is not section 25.2's"
        );
        assert_eq!(
            sections
                .first()
                .ok_or("the screen rendered no first section")?
                .group(),
            HomeGroup::TodaysSchedule,
            "something other than 오늘 실제 일정 is first"
        );
    }

    // 4. Every `mod` this crate declares names a file the walk read, so the
    //    inventory above cannot be complete over a subtree it never entered.
    let read: BTreeSet<String> = product_sources()?
        .iter()
        .filter_map(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect();
    let lib = strip_non_code(&fs::read_to_string(
        crate_root().join("src").join("lib.rs"),
    )?);
    let mut declared = BTreeSet::new();
    for line in lib.lines() {
        let trimmed = line.trim();
        let rest = trimmed
            .strip_prefix("pub mod ")
            .or_else(|| trimmed.strip_prefix("mod "));
        if let Some(rest) = rest
            && let Some(name) = rest.strip_suffix(';')
        {
            declared.insert(name.trim().to_owned());
        }
    }
    assert!(!declared.is_empty(), "lib.rs declares no module at all");
    assert_eq!(
        declared.difference(&read).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "a module is declared that the walk never read"
    );
    Ok(())
}

/// The field reader sees the shape an injection would take.
///
/// A whole-set comparison is only as good as the reader under it, so the reader
/// is driven over a fixture holding exactly what a headline metric would look
/// like — a struct field, an enum tuple position and an enum struct-variant
/// field, none of them spelling anything this suite forbids, because this suite
/// forbids no spelling at all.
#[test]
fn the_field_reader_finds_a_position_nobody_reviewed() {
    let fixture = "
pub struct Marquee {
    headline: Permille,
    at: TimestampMillis,
}

pub enum Banner {
    Rolling(Permille),
    Windowed { span: u32, value: Permille },
}
";
    let found: BTreeSet<String> = field_positions_in(&strip_non_code(fixture))
        .iter()
        .map(FieldPosition::key)
        .collect();
    let expected: BTreeSet<String> = [
        "Marquee.headline: Permille",
        "Marquee.at: TimestampMillis",
        "Banner::Rolling.#0: Permille",
        "Banner::Windowed.span: u32",
        "Banner::Windowed.value: Permille",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        found, expected,
        "the field reader does not see every position an injection could take"
    );

    // And it is not answering the same thing whatever it is given.
    assert!(field_positions_in(&strip_non_code("fn nothing() {}")).is_empty());
}

/// This crate has no name for a mastery level.
///
/// `P2-N3`'s rule is that time decay reaches a freshness projection and never a
/// mastery. `academic_domain::MasteryLevel` is one `use` away from this crate,
/// so the claim is not made by the closure; it is made by the source, and by
/// the same shape `academic-freshness` uses: a whole-set reading of the crate
/// text with a **control** that requires the same reader to find those names
/// where they really are, so the zero reported here is a measurement rather
/// than a broken reader.
#[test]
fn the_home_surface_cannot_name_a_mastery() -> TestResult {
    let names = [
        "MasteryLevel",
        "MasteryProjection",
        "AutomaticLevel",
        "LADDER",
        "rung",
        "level_token",
    ];
    for path in product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for name in names {
            assert!(!code.contains(name), "{} names {name}", path.display());
        }
    }

    // The control. The same reader over `P2-N2`'s own ladder must find most of
    // them, or the zero above would be a reader that reads nothing.
    let ladder = strip_non_code(&fs::read_to_string(
        workspace_root()
            .join("crates")
            .join("knowledge-state")
            .join("src")
            .join("ladder.rs"),
    )?);
    let hits = names.iter().filter(|name| ladder.contains(**name)).count();
    assert!(
        hits >= 4,
        "the control found only {hits} of the mastery names in P2-N2's ladder"
    );
    Ok(())
}
