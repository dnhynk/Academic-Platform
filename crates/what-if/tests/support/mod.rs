//! Shared fixtures for the `P2-N8` acceptance suite.
//!
//! Every value here is synthetic and built in process. Nothing reads a real
//! record, a real catalogue or a real review, and no identifier is a function
//! of a clock: each is a version-seven-shaped UUID derived from its own
//! fixture suffix.
//!
//! The two values that could have been transcribed are not. A
//! [`academic_offering::ConfirmedSeat`] is built by driving `P2-U5`'s own
//! `resolve` over an official registration-system reading, because that crate's
//! `ConfirmedStanding::seat` is the only producer of one in this workspace; and
//! the bias disclosure is built through `P2-U8`'s own draft, which refuses a
//! disclosure that leaves any of section 29.5's six dimensions unnamed.

#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_critical_path::PathRole;
use academic_curriculum::{
    Capacity, CourseCode, Credits, CurriculumCategory, InstructorName, Meeting,
    OfficialPrerequisite, Weekday,
};
use academic_domain::{ContentDigest, CourseId, EntityId, ModelRunId, OfferingId, TimestampMillis};
use academic_ingestion::{ConnectorId, SourceCategory};
use academic_offering::{
    ConfirmationEvidence, ConfirmedSeat, OfficialListing, OfficialTermReading, corpus, resolve,
};
use academic_record::{
    grade::{GradeSymbol, GradingScheme},
    term::{Semester, TermKey},
};
use academic_review::{
    BiasDimension, BiasDisclosure, BiasDisclosureDraft, BiasFinding, BiasStrength,
};
use academic_scenario::{
    LikelihoodBand, OpportunityBasis, SyllabusConceptSignal, WorkloadHoursRange,
};
use academic_what_if::{
    AssumedWorkload, CatalogueRow, DownstreamCourse, EnrolmentLimitStanding,
    HypotheticalCompletion, InformalRecommendation, OfficialConditions, PLAN_CHOICE_FIELDS,
    PLAN_INPUT_FIELDS, PathCoverageTargets, PlanAssumptions, PlanChoice, PlanChoiceField,
    PlanInputField, PlanInputs, ProbabilisticCoverage, RelevanceSignal, RelevanceSubject,
    ScenarioBasis, StatedGradeAssumption, StatedGradeAssumptions, WhatIfError,
};

pub type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------
// identifiers
// ---------------------------------------------------------------------------

fn identifier(suffix: u32) -> String {
    format!("01900000-0000-7000-8000-0000{suffix:08x}")
}

pub fn entity(suffix: u32) -> Result<EntityId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<EntityId>()?)
}

pub fn offering(suffix: u32) -> Result<OfferingId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<OfferingId>()?)
}

pub fn course(suffix: u32) -> Result<CourseId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<CourseId>()?)
}

pub fn model_run(suffix: u32) -> Result<ModelRunId, Box<dyn Error>> {
    Ok(identifier(suffix).parse::<ModelRunId>()?)
}

#[must_use]
pub fn digest(byte: u8) -> ContentDigest {
    ContentDigest::from_sha256_bytes([byte; 32])
}

#[must_use]
pub fn now() -> TimestampMillis {
    TimestampMillis::new(corpus::CORPUS_NOW_MILLIS)
}

// ---------------------------------------------------------------------------
// `P2-U5`'s confirmed seat
// ---------------------------------------------------------------------------

pub fn spring_2026() -> Result<TermKey, Box<dyn Error>> {
    Ok(TermKey::new(2026, Semester::Spring)?)
}

/// A confirmed seat with the timetable the caller names.
///
/// Driven through `P2-U5`'s own `resolve`, so the seat this suite plans over is
/// the seat that crate produces and not a shape this suite invented. The
/// forecast policy is `None`: no prediction runs, and the standing is decided
/// by the official registration-system reading alone.
pub fn seat(code: &str, meetings: &[Meeting]) -> Result<ConfirmedSeat, Box<dyn Error>> {
    let retrieved_at = TimestampMillis::new(corpus::CORPUS_NOW_MILLIS - 3_600_000);
    let mut listing = OfficialListing::new(
        SourceCategory::RegistrationSystem,
        ConnectorId::new("sugang.snu.ac.kr")?,
        retrieved_at,
        spring_2026()?,
        CourseCode::parse(code)?,
        true,
    )
    .instructor(InstructorName::parse("Instructor A")?)
    .capacity(Capacity::new(60));
    for meeting in meetings {
        listing = listing.meeting(*meeting);
    }
    let evidence = ConfirmationEvidence::from_registration_system(
        listing,
        Vec::new(),
        corpus::verification_recency()?,
        now(),
    )?;
    let reading = OfficialTermReading::Confirmed(evidence);
    let resolution = resolve(
        &corpus::history("every_spring")?,
        corpus::window("every_spring")?,
        Some(&reading),
        None,
        &corpus::calibration_registry((corpus::CORPUS_NOW_MILLIS - 3_600_000) as u64)?,
        now(),
    )?;
    resolution
        .standing()
        .seat()
        .ok_or_else(|| "the confirmed reading produced no seat".into())
}

pub fn meeting(weekday: Weekday, from: u16, to: u16) -> Result<Meeting, Box<dyn Error>> {
    Ok(Meeting::new(weekday, from, to)?)
}

// ---------------------------------------------------------------------------
// `P2-U8`'s bias disclosure
// ---------------------------------------------------------------------------

/// A disclosure naming every one of section 29.5's six dimensions.
pub fn bias() -> Result<BiasDisclosure, Box<dyn Error>> {
    let mut draft = BiasDisclosureDraft::new();
    for (dimension, measured, strength) in [
        (BiasDimension::SampleCount, 14_u32, BiasStrength::Elevated),
        (BiasDimension::Recency, 3, BiasStrength::Low),
        (BiasDimension::InstructorTermMix, 2, BiasStrength::Elevated),
        (BiasDimension::SelfSelection, 14, BiasStrength::Severe),
        (BiasDimension::ExtremeExperience, 4, BiasStrength::Elevated),
        (BiasDimension::Duplication, 1, BiasStrength::Low),
    ] {
        draft = draft.disclosing(BiasFinding::new(dimension, measured, strength));
    }
    Ok(draft.build()?)
}

// ---------------------------------------------------------------------------
// the plan
// ---------------------------------------------------------------------------

pub fn basis() -> ScenarioBasis {
    ScenarioBasis::of(
        digest(0x11),
        digest(0x22),
        digest(0x33),
        TimestampMillis::new(7_000),
    )
}

pub fn assumptions() -> Result<PlanAssumptions, Box<dyn Error>> {
    Ok(PlanAssumptions::of(
        AssumedWorkload::of(WorkloadHoursRange::new(34, 46)?, model_run(9001)?),
        HypotheticalCompletion,
        ProbabilisticCoverage,
    ))
}

/// The concept identifiers the fixtures share.
pub struct Concepts {
    pub tcp: EntityId,
    pub disk_page: EntityId,
    pub isolation: EntityId,
    pub idempotency: EntityId,
}

pub fn concepts() -> Result<Concepts, Box<dyn Error>> {
    Ok(Concepts {
        tcp: entity(2001)?,
        disk_page: entity(2002)?,
        isolation: entity(2003)?,
        idempotency: entity(2004)?,
    })
}

pub fn signal(
    concept: EntityId,
    basis: OpportunityBasis,
    coverage_permille: u16,
    assessed: bool,
) -> SyllabusConceptSignal {
    SyllabusConceptSignal {
        concept_entity_id: concept,
        basis,
        coverage_permille,
        assessed,
    }
}

/// Plan A: networks and databases, no timetable conflict.
pub fn plan_a() -> Result<PlanInputs, Box<dyn Error>> {
    let names = concepts()?;
    let networks = PlanChoice::of(
        offering(1001)?,
        seat("M1522.001800", &[meeting(Weekday::Tuesday, 570, 645)?])?,
        CatalogueRow::of(
            course(3001)?,
            Credits::new(3)?,
            CurriculumCategory::MajorElective,
        ),
        OfficialConditions::of(
            vec![OfficialPrerequisite::on(course(3901)?)],
            EnrolmentLimitStanding::Satisfied,
        ),
        WorkloadHoursRange::new(12, 20)?,
        vec![
            signal(names.tcp, OpportunityBasis::Syllabus, 400, true),
            signal(
                names.idempotency,
                OpportunityBasis::AssignmentBrief,
                150,
                false,
            ),
        ],
    );
    let databases = PlanChoice::of(
        offering(1002)?,
        seat("M1522.002300", &[meeting(Weekday::Thursday, 570, 645)?])?,
        CatalogueRow::of(
            course(3002)?,
            Credits::new(3)?,
            CurriculumCategory::MajorRequired,
        ),
        OfficialConditions::of(Vec::new(), EnrolmentLimitStanding::Unknown),
        WorkloadHoursRange::new(14, 22)?,
        vec![
            signal(names.disk_page, OpportunityBasis::Syllabus, 250, false),
            signal(
                names.isolation,
                OpportunityBasis::AssignmentBrief,
                320,
                true,
            ),
        ],
    );
    Ok(PlanInputs {
        scenario_id: entity(4001)?,
        model_run_id: model_run(9001)?,
        basis: basis(),
        assumptions: assumptions()?,
        choices: vec![networks, databases],
        completed_courses: [course(3901)?].into_iter().collect::<BTreeSet<_>>(),
        downstream_courses: vec![
            DownstreamCourse::of(
                course(3101)?,
                vec![
                    OfficialPrerequisite::on(course(3001)?),
                    OfficialPrerequisite::on(course(3002)?),
                ],
            ),
            DownstreamCourse::of(course(3102)?, vec![OfficialPrerequisite::on(course(3801)?)]),
        ],
        path_targets: PathCoverageTargets::of(vec![
            (names.isolation, PathRole::SharedSpine),
            (names.idempotency, PathRole::OptionalBranch),
        ])?,
        relevance: vec![
            RelevanceSignal::of(
                offering(1002)?,
                RelevanceSubject::Project,
                entity(5001)?,
                LikelihoodBand::High,
                OpportunityBasis::Syllabus,
            ),
            RelevanceSignal::of(
                offering(1001)?,
                RelevanceSubject::Career,
                entity(5002)?,
                LikelihoodBand::Moderate,
                OpportunityBasis::HistoricalOffering,
            ),
        ],
        informal_recommendations: vec![
            InformalRecommendation::of(entity(6001)?, vec![names.isolation, names.idempotency]),
            InformalRecommendation::of(entity(6002)?, vec![entity(6100)?]),
        ],
        workload_bias: bias()?,
        grade_assumptions: None,
        grading_scheme: GradingScheme::snu_4_3_v1()?,
    })
}

/// Plan B: a lighter, conflicting pair with less path coverage.
pub fn plan_b() -> Result<PlanInputs, Box<dyn Error>> {
    let names = concepts()?;
    let mut inputs = plan_a()?;
    inputs.scenario_id = entity(4002)?;
    let algorithms = PlanChoice::of(
        offering(1003)?,
        seat("M1522.003100", &[meeting(Weekday::Tuesday, 600, 700)?])?,
        CatalogueRow::of(
            course(3003)?,
            Credits::new(3)?,
            CurriculumCategory::MajorElective,
        ),
        OfficialConditions::of(Vec::new(), EnrolmentLimitStanding::Satisfied),
        WorkloadHoursRange::new(8, 14)?,
        vec![signal(
            names.tcp,
            OpportunityBasis::HistoricalOffering,
            90,
            false,
        )],
    );
    let networks = inputs
        .choices
        .first()
        .cloned()
        .ok_or("plan A carries no first choice")?;
    inputs.choices = vec![networks, algorithms];
    inputs.relevance = vec![RelevanceSignal::of(
        offering(1001)?,
        RelevanceSubject::Career,
        entity(5002)?,
        LikelihoodBand::Moderate,
        OpportunityBasis::HistoricalOffering,
    )];
    Ok(inputs)
}

/// Grades stated for every choice of plan A.
pub fn stated_grades() -> Result<StatedGradeAssumptions, WhatIfError> {
    StatedGradeAssumptions::stating(vec![
        StatedGradeAssumption::of(
            identifier(1001)
                .parse::<OfferingId>()
                .map_err(WhatIfError::Domain)?,
            GradeSymbol::AZero,
        ),
        StatedGradeAssumption::of(
            identifier(1002)
                .parse::<OfferingId>()
                .map_err(WhatIfError::Domain)?,
            GradeSymbol::BPlus,
        ),
    ])
}

// ---------------------------------------------------------------------------
// reading the specification and this crate's own source
// ---------------------------------------------------------------------------

#[must_use]
pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[must_use]
pub fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

pub fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// One `### `-headed block of the specification, up to the next one.
pub fn section(specification: &str, heading: &str) -> Result<String, Box<dyn Error>> {
    let start = specification
        .find(heading)
        .ok_or_else(|| format!("the specification has no {heading} heading"))?;
    let rest = &specification[start + heading.len()..];
    let end = rest
        .find("\n### ")
        .or_else(|| rest.find("\n## "))
        .ok_or_else(|| format!("{heading} does not end at a following heading"))?;
    Ok(rest[..end].to_owned())
}

/// The `- ` bullets of a block, in order, stopping at the first blank-line
/// gap after the list starts.
#[must_use]
pub fn bullets(block: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(text) = trimmed.strip_prefix("- ") {
            found.push(text.trim().to_owned());
        } else if !found.is_empty() && !trimmed.is_empty() && !trimmed.starts_with("- ") {
            break;
        }
    }
    found
}

/// Every `.rs` file the package ships: the whole package outside `tests`.
pub fn product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
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
    if found.len() < 12 {
        return Err(format!("the walk found only {} product files", found.len()).into());
    }
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

/// Comments, string literals and character literals removed.
///
/// A scan over raw text would find a forbidden word in a doc comment that
/// explains why the word is forbidden, which is the false positive that makes
/// a source scan get weakened until it finds nothing at all.
#[must_use]
pub fn strip_non_code(source: &str) -> String {
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
        out.push(current);
        index += 1;
    }
    out
}

/// Every whole identifier in a source text.
#[must_use]
pub fn identifiers(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut current = String::new();
    for character in source.chars() {
        if character.is_alphanumeric() || character == '_' {
            current.push(character);
        } else {
            if !current.is_empty() {
                found.insert(std::mem::take(&mut current));
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        found.insert(current);
    }
    found
}

/// The block a top-level `impl` or `struct` header opens, up to the `}` that
/// closes it at column zero.
///
/// `cargo fmt --check` runs in the required verification block, so a top-level
/// item's closing brace is at column zero and this slice is exact. A header the
/// file does not hold raises rather than returning an empty block: an empty
/// block would make every comparison below pass over nothing.
pub fn block_of(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("the source has no {header}"))?;
    let rest = &source[start..];
    let end = rest
        .find("\n}\n")
        .ok_or_else(|| format!("{header} does not close at column zero"))?;
    Ok(rest[..end].to_owned())
}

/// The `name: Type` field declarations of one block, in order.
#[must_use]
pub fn field_declarations(block: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for line in block.lines().skip(1) {
        let trimmed = line.trim();
        let Some((name, rest)) = trimmed.split_once(':') else {
            continue;
        };
        let name = name.trim().trim_start_matches("pub ").trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
        {
            continue;
        }
        let Some(kind) = rest.strip_suffix(',') else {
            continue;
        };
        found.push((name.to_owned(), kind.trim().to_owned()));
    }
    found
}

/// The `fn` names declared in one block, in order.
#[must_use]
pub fn function_names(block: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("pub fn ")
            .or_else(|| trimmed.strip_prefix("pub const fn "))
            .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
            .or_else(|| trimmed.strip_prefix("pub(crate) const fn "))
            .or_else(|| trimmed.strip_prefix("fn "))
            .or_else(|| trimmed.strip_prefix("const fn "))
        else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            found.push(name);
        }
    }
    found
}

/// One module's source text.
pub fn module_source(name: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(
        crate_root().join("src").join(format!("{name}.rs")),
    )?)
}

/// Every field position this crate declares, as `Type.field: Type` mapped to
/// its visibility.
///
/// A structural walk over the whole package rather than a list: the parse finds
/// a `struct` header at column zero in any product file, in any module, whether
/// or not anybody predicted the module. That is what makes an inventory
/// comparison against this an exhaustive net rather than a list of names.
pub fn declared_field_positions() -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let mut found = BTreeMap::new();
    for path in product_sources()? {
        let source = fs::read_to_string(&path)?;
        let mut rest = source.as_str();
        let mut offset = 0_usize;
        while let Some(at) = rest[offset..].find("struct ") {
            let index = offset + at;
            let line_start = rest[..index].rfind('\n').map_or(0, |position| position + 1);
            let prefix = &rest[line_start..index];
            if !(prefix.is_empty() || prefix == "pub " || prefix == "pub(crate) ") {
                offset = index + "struct ".len();
                continue;
            }
            let after = &rest[index + "struct ".len()..];
            let name: String = after
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            let Some(open) = after.find('{') else {
                break;
            };
            let Some(close) = after.find("\n}\n") else {
                break;
            };
            if open < close {
                for line in after[open..close].lines().skip(1) {
                    let trimmed = line.trim();
                    let (visibility, declaration) = trimmed
                        .strip_prefix("pub ")
                        .map_or(("private", trimmed), |declaration| ("pub", declaration));
                    let Some((field, kind)) = declaration.split_once(": ") else {
                        continue;
                    };
                    if field.is_empty()
                        || !field
                            .chars()
                            .all(|character| character.is_alphanumeric() || character == '_')
                    {
                        continue;
                    }
                    let Some(kind) = kind.strip_suffix(',') else {
                        continue;
                    };
                    found.insert(format!("{name}.{field}: {kind}"), visibility.to_owned());
                }
            }
            offset = index + "struct ".len();
        }
        rest = &source;
        let _ = rest;
    }
    if found.len() < 100 {
        return Err(format!("the field walk found only {} positions", found.len()).into());
    }
    Ok(found)
}

/// One variation of [`plan_a`] per field of `PlanInputs`, each changing that
/// field and nothing else.
///
/// The list is built by walking `PLAN_INPUT_FIELDS` with a total `match`, so a
/// field added to the enumeration stops this module compiling until it says
/// what varying it looks like. That is the guard against the shape `P2-N6`
/// found in its own frozen inputs: a field the engine reads that the digest
/// never covered.
pub fn input_variants() -> Result<Vec<(&'static str, PlanInputs)>, Box<dyn Error>> {
    let names = concepts()?;
    let mut variants = Vec::new();
    for field in PLAN_INPUT_FIELDS {
        let mut inputs = plan_a()?;
        match field {
            PlanInputField::ScenarioId => inputs.scenario_id = entity(4900)?,
            PlanInputField::ModelRunId => inputs.model_run_id = model_run(9900)?,
            PlanInputField::Basis => {
                inputs.basis = ScenarioBasis::of(
                    digest(0x44),
                    digest(0x22),
                    digest(0x33),
                    TimestampMillis::new(7_000),
                );
            }
            PlanInputField::Assumptions => {
                inputs.assumptions = PlanAssumptions::of(
                    AssumedWorkload::of(WorkloadHoursRange::new(30, 40)?, model_run(9001)?),
                    HypotheticalCompletion,
                    ProbabilisticCoverage,
                );
            }
            PlanInputField::Choices => {
                inputs.choices.truncate(1);
            }
            PlanInputField::CompletedCourses => {
                inputs.completed_courses = BTreeSet::new();
            }
            PlanInputField::DownstreamCourses => {
                inputs.downstream_courses.truncate(1);
            }
            PlanInputField::PathTargets => {
                inputs.path_targets =
                    PathCoverageTargets::of(vec![(names.isolation, PathRole::AlternativePath)])?;
            }
            PlanInputField::Relevance => {
                inputs.relevance.truncate(1);
            }
            PlanInputField::InformalRecommendations => {
                inputs.informal_recommendations.truncate(1);
            }
            PlanInputField::WorkloadBias => {
                let mut draft = BiasDisclosureDraft::new();
                for (dimension, measured, strength) in [
                    (BiasDimension::SampleCount, 99_u32, BiasStrength::Low),
                    (BiasDimension::Recency, 3, BiasStrength::Low),
                    (BiasDimension::InstructorTermMix, 2, BiasStrength::Elevated),
                    (BiasDimension::SelfSelection, 14, BiasStrength::Severe),
                    (BiasDimension::ExtremeExperience, 4, BiasStrength::Elevated),
                    (BiasDimension::Duplication, 1, BiasStrength::Low),
                ] {
                    draft = draft.disclosing(BiasFinding::new(dimension, measured, strength));
                }
                inputs.workload_bias = draft.build()?;
            }
            PlanInputField::GradeAssumptions => {
                inputs.grade_assumptions = Some(stated_grades()?);
            }
            PlanInputField::GradingScheme => {
                inputs.grading_scheme = GradingScheme::snu_4_3_v2_scale3()?;
            }
        }
        variants.push((field.field_name(), inputs));
    }
    Ok(variants)
}

/// One variation of plan A's first choice per field of `PlanChoice`, each
/// changing that field and nothing else.
pub fn choice_variants() -> Result<Vec<(&'static str, PlanInputs)>, Box<dyn Error>> {
    let names = concepts()?;
    let mut variants = Vec::new();
    for field in PLAN_CHOICE_FIELDS {
        let mut inputs = plan_a()?;
        let original = inputs
            .choices
            .first()
            .cloned()
            .ok_or("plan A carries no first choice")?;
        let replacement = match field {
            PlanChoiceField::OfferingId => PlanChoice::of(
                offering(1900)?,
                original.seat().clone(),
                CatalogueRow::of(original.course(), original.credits(), original.category()),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    original.enrolment_limit(),
                ),
                original.assumed_weekly_hours(),
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::Course => PlanChoice::of(
                original.offering_id(),
                original.seat().clone(),
                CatalogueRow::of(course(3900)?, original.credits(), original.category()),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    original.enrolment_limit(),
                ),
                original.assumed_weekly_hours(),
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::Seat => PlanChoice::of(
                original.offering_id(),
                seat("M1522.009900", &[meeting(Weekday::Monday, 600, 700)?])?,
                CatalogueRow::of(original.course(), original.credits(), original.category()),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    original.enrolment_limit(),
                ),
                original.assumed_weekly_hours(),
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::Credits => PlanChoice::of(
                original.offering_id(),
                original.seat().clone(),
                CatalogueRow::of(original.course(), Credits::new(4)?, original.category()),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    original.enrolment_limit(),
                ),
                original.assumed_weekly_hours(),
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::Category => PlanChoice::of(
                original.offering_id(),
                original.seat().clone(),
                CatalogueRow::of(
                    original.course(),
                    original.credits(),
                    CurriculumCategory::GeneralElective,
                ),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    original.enrolment_limit(),
                ),
                original.assumed_weekly_hours(),
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::OfficialPrerequisites => PlanChoice::of(
                original.offering_id(),
                original.seat().clone(),
                CatalogueRow::of(original.course(), original.credits(), original.category()),
                OfficialConditions::of(Vec::new(), original.enrolment_limit()),
                original.assumed_weekly_hours(),
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::EnrolmentLimit => PlanChoice::of(
                original.offering_id(),
                original.seat().clone(),
                CatalogueRow::of(original.course(), original.credits(), original.category()),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    EnrolmentLimitStanding::NotSatisfied,
                ),
                original.assumed_weekly_hours(),
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::AssumedWeeklyHours => PlanChoice::of(
                original.offering_id(),
                original.seat().clone(),
                CatalogueRow::of(original.course(), original.credits(), original.category()),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    original.enrolment_limit(),
                ),
                WorkloadHoursRange::new(13, 21)?,
                original.syllabus_concepts().to_vec(),
            ),
            PlanChoiceField::SyllabusConcepts => PlanChoice::of(
                original.offering_id(),
                original.seat().clone(),
                CatalogueRow::of(original.course(), original.credits(), original.category()),
                OfficialConditions::of(
                    original.official_prerequisites().to_vec(),
                    original.enrolment_limit(),
                ),
                original.assumed_weekly_hours(),
                vec![signal(
                    names.tcp,
                    OpportunityBasis::AssessmentPlan,
                    700,
                    true,
                )],
            ),
        };
        if let Some(first) = inputs.choices.first_mut() {
            *first = replacement;
        }
        variants.push((field.field_name(), inputs));
    }
    Ok(variants)
}

/// The refusal a call produced, or a failure naming what it produced instead.
///
/// `unwrap_err` is denied workspace-wide, and a test that panicked on an
/// unexpected success would report a panic rather than the value that was
/// wrongly admitted.
pub fn refusal<T: std::fmt::Debug>(
    result: Result<T, WhatIfError>,
) -> Result<WhatIfError, Box<dyn Error>> {
    match result {
        Ok(value) => Err(format!("expected a refusal, got {value:?}").into()),
        Err(error) => Ok(error),
    }
}

/// Every workspace package `root` reaches through a declared edge of any kind.
///
/// Judged from the manifests rather than from the source text, for the reason
/// `P2-C7`'s own writer gate gives: a source grep passes for a crate that links
/// the writer and simply has not mentioned it yet, which is the state one edit
/// away from a leak. Dev edges are walked too, because a dev edge is still a
/// compiled edge and a case could name the writer through one.
pub fn declared_closure(root: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let crates = workspace_root().join("crates");
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut entries = 0_usize;
    for entry in fs::read_dir(&crates)? {
        let manifest = entry?.path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let text = fs::read_to_string(&manifest)?;
        let name = text
            .lines()
            .find_map(|line| line.strip_prefix("name = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .ok_or_else(|| format!("{} declares no package name", manifest.display()))?
            .to_owned();
        let mut edges = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.contains("path = \"../") {
                continue;
            }
            let Some((declared, _)) = trimmed.split_once(' ') else {
                continue;
            };
            if declared.starts_with("academic-") {
                edges.push(declared.to_owned());
            }
        }
        entries += 1;
        by_name.insert(name, edges);
    }
    assert!(
        entries > 40,
        "the manifest walk found only {entries} workspace packages"
    );
    let mut reached = BTreeSet::new();
    let mut pending = vec![root.to_owned()];
    while let Some(name) = pending.pop() {
        let Some(edges) = by_name.get(&name) else {
            continue;
        };
        for edge in edges {
            if reached.insert(edge.clone()) {
                pending.push(edge.clone());
            }
        }
    }
    Ok(reached)
}

/// Every type one module declares: `struct`, `enum` and `type` alike.
///
/// Derived rather than listed, so a type added to one lane's module extends the
/// other lane's refusal without anybody editing a test.
#[must_use]
pub fn declared_type_names(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in strip_non_code(source).lines() {
        for keyword in [
            "pub struct ",
            "pub enum ",
            "pub type ",
            "struct ",
            "enum ",
            "type ",
        ] {
            let Some(rest) = line.strip_prefix(keyword) else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !name.is_empty() {
                found.insert(name);
            }
            break;
        }
    }
    found
}

/// Every leaf one module imports from one crate root.
#[must_use]
pub fn imported_leaf_names(source: &str, root: &str) -> BTreeSet<String> {
    let code = strip_non_code(source);
    let mut found = BTreeSet::new();
    let opener = format!("use {root}::");
    let mut rest = code.as_str();
    while let Some(at) = rest.find(&opener) {
        let after = &rest[at + opener.len()..];
        let Some(end) = after.find(';') else {
            break;
        };
        for leaf in after[..end]
            .replace(['{', '}'], "")
            .split(',')
            .map(str::trim)
        {
            if !leaf.is_empty()
                && leaf
                    .chars()
                    .all(|character| character.is_alphanumeric() || character == '_')
            {
                found.insert(leaf.to_owned());
            }
        }
        rest = &after[end..];
    }
    found
}

/// Whether a field's declared type names one type, as a whole identifier.
///
/// Substring matching would report `PathCoverage` for a field typed
/// `PathCoverageEntry`, which is true but useless; whole-identifier matching is
/// what makes the emptiness above a real claim.
#[must_use]
pub fn names_type(declaration: &str, name: &str) -> bool {
    identifiers(declaration).contains(name)
}

/// Every function in one source that hands out a bare number, as
/// `module.rs name -> type`.
///
/// The return type only, never a wrapper: an `Option<usize>` or a
/// `Result<Decimal, _>` still hands out a number, and both are matched here
/// because the inner type is what a caller reads.
#[must_use]
pub fn numeric_returns(module: &str, source: &str) -> BTreeSet<String> {
    const NUMBERS: [&str; 11] = [
        "u8", "u16", "u32", "u64", "usize", "i32", "i64", "i128", "f32", "f64", "Decimal",
    ];
    let code = strip_non_code(source);
    let mut found = BTreeSet::new();
    let mut rest = code.as_str();
    while let Some(at) = rest.find("fn ") {
        let after = &rest[at + 3..];
        let name: String = after
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        let Some(arrow) = after.find("->") else {
            break;
        };
        let Some(open) = after.find('{') else {
            break;
        };
        if !name.is_empty() && arrow < open {
            let returned = &after[arrow + 2..open];
            for number in NUMBERS {
                if identifiers(returned).contains(number) {
                    found.insert(format!("{module} {name} -> {number}"));
                }
            }
        }
        rest = after;
    }
    found
}
