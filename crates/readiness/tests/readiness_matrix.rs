//! `P2-Y3`'s acceptance suite: section 24.3's career readiness view.
//!
//! Seven of the eight tests the execution plan names are here; the eighth,
//! `non_guarantee_disclaimer_survives_export`, is in
//! `crates/readiness/tests/readiness_export.rs` because it writes and reads a
//! real `P2-P1` bundle. The rest are the measurements those eight rest on.
//!
//! # Nothing here fabricates a value another crate owns
//!
//! Every competency is declared through `P2-Y1`'s own `declare`, every bundle
//! through `P2-Y2`'s own `declare`, every knowledge-state item through
//! `P2-N2`'s own four eligibility checks, and every personal application claim
//! through the whole `P2-R1` → `P2-R5` chain. See `tests/support/mod.rs`.
//!
//! # The specification is read, not restated
//!
//! Seven readings come out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`
//! at run time rather than being copied into this file: section 24.3's table
//! header row, section 36.9's per-competency block, section 24.3's evidence-stage
//! sentence, section 24.3's disclosure sentence, section 24.4's four bullets,
//! section 34.5's career row and section 35's anti-goal row. Each is compared
//! **in both directions**, so a specification that renames, adds or drops one
//! fails this suite instead of drifting past it. `P2-N6` set that pattern and
//! `P2-N3`, `P2-Y1` and `P2-Y2` follow it.
//!
//! # The two sixes
//!
//! Section 24.3 states six columns in its table and six evidence stages in its
//! prose, and they are **different sets**. This suite reads both and requires
//! them to be unequal, so the reading is recorded and executed rather than
//! assumed. See `docs/contracts/career-readiness-matrix.md`.

use std::{collections::BTreeSet, fs};

use academic_competency::{Competency, EvidenceStage};
use academic_domain::FreshnessBand;
use academic_readiness::{
    AbsenceState, AuxiliaryScore, AxisCell, AxisEvidence, AxisWeight, CompetencyInput,
    EvidenceLocatorId, MissingDataDisclosure, NavigationDirection, NonGuaranteeNotice,
    ReadinessAxis, ReadinessError, ReadinessEvent, ReadinessMatrix, ReadinessView,
    RubricDisclosure, ScoreValue, SourceDisclosure, StartingPoint, StartingPointId, Terminus,
    UnknownBasis, ViewBlock, WeightDisclosure, disclose, take, traverse,
};
use academic_role_profile::{BundleImportance, RoleProfile};

mod support;

use support::{
    TestResult, bundle, competency_about, design_page, entry, knowledge_evidence_id,
    knowledge_record, ontology, personal_claim, personal_record, placed, section,
};

// ---------------------------------------------------------------------------
// The design document's own readings.
// ---------------------------------------------------------------------------

/// Section 24.3's table header cells, after `Competency`.
fn spec_table_columns(page: &str) -> TestResult<Vec<String>> {
    let body = section(page, "### 24.3 Career Readiness View", "### 24.4")?;
    let header = body
        .lines()
        .find(|line| line.starts_with("| Competency |"))
        .ok_or("section 24.3 has no matrix header row")?;
    Ok(header
        .trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .skip(1)
        .filter(|cell| !cell.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Section 36.9's per-competency block keys, in the block's own order.
fn spec_scenario_keys(page: &str) -> TestResult<Vec<String>> {
    let body = section(page, "### 36.9", "## 37.")?;
    let start = body
        .find("```text")
        .ok_or("section 36.9 has no career view block")?;
    let rest = &body[start + "```text".len()..];
    let end = rest
        .find("```")
        .ok_or("section 36.9's block does not end")?;
    Ok(rest[..end]
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, _)| key.trim().to_owned())
        .filter(|key| !key.is_empty())
        .collect())
}

/// Section 24.3's back-quoted evidence-stage names, in its own order.
fn spec_stage_names(page: &str) -> TestResult<Vec<String>> {
    let body = section(page, "### 24.3 Career Readiness View", "### 24.4")?;
    let sentence = body
        .lines()
        .find(|line| line.contains("evidence를 구분한다"))
        .ok_or("section 24.3 does not name its evidence stages")?;
    Ok(sentence
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect())
}

/// Section 24.4's `- ` bullets, in the section's own order.
fn spec_direction_bullets(page: &str) -> TestResult<Vec<String>> {
    let body = section(page, "### 24.4 ", "---")?;
    Ok(body
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|line| line.trim().to_owned())
        .collect())
}

/// Section 34.5's `career readiness 과도한 점수화` row, cell by cell.
fn spec_career_failure_row(page: &str) -> TestResult<Vec<String>> {
    let body = section(page, "### 34.5 ", "### 34.6")?;
    let row = body
        .lines()
        .find(|line| line.contains("career readiness 과도한 점수화"))
        .ok_or("section 34.5 has no career readiness row")?;
    Ok(row
        .trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect())
}

/// Section 35's `LinkedIn식 career scoring` row, cell by cell.
fn spec_anti_goal_row(page: &str) -> TestResult<Vec<String>> {
    let body = section(page, "## 35. Anti-goals", "## 36.")?;
    let row = body
        .lines()
        .find(|line| line.contains("LinkedIn"))
        .ok_or("section 35 has no LinkedIn row")?;
    Ok(row
        .trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect())
}

/// Section 24.3's last sentence, which is the disclosure contract.
fn spec_disclosure_sentence(page: &str) -> TestResult<String> {
    let body = section(page, "### 24.3 Career Readiness View", "### 24.4")?;
    body.lines()
        .find(|line| line.contains("보조 score가 필요하면"))
        .map(str::to_owned)
        .ok_or_else(|| "section 24.3 states no disclosure sentence".into())
}

// ---------------------------------------------------------------------------
// The fixture: one bundle, three competencies, one of them unsupplied.
// ---------------------------------------------------------------------------

const DATABASE: &str = "relational-database-diagnosis";
const AUTHENTICATION: &str = "authentication";
const DISTRIBUTED: &str = "distributed-failure-reasoning";

const INDEXING: &str = "chooses-an-index";
const MEASURING: &str = "measures-before-optimizing";

struct Fixture {
    database: Competency,
    authentication: Competency,
    distributed: Competency,
    bundle: RoleProfile,
    placements: Vec<AxisEvidence>,
}

impl Fixture {
    fn competencies(&self) -> Vec<&Competency> {
        vec![&self.database, &self.authentication, &self.distributed]
    }

    /// The matrix of the whole bundle, with the first two competencies supplied
    /// and the third deliberately absent.
    fn matrix(&self) -> ReadinessMatrix {
        let inputs = vec![
            CompetencyInput::of(&self.database, &self.placements, FreshnessBand::High),
            CompetencyInput::of(&self.authentication, &[], FreshnessBand::Moderate),
        ];
        take(&self.bundle, &inputs)
    }

    fn view(&self) -> Result<ReadinessView, ReadinessError> {
        ReadinessView::of(self.matrix(), &self.competencies())
    }

    /// The same bundle with the third competency supplied by two placements it
    /// refuses, one for each [`UnknownBasis`].
    fn view_with_refusals(
        &self,
        refused: &[AxisEvidence],
    ) -> Result<ReadinessView, ReadinessError> {
        let inputs = vec![
            CompetencyInput::of(&self.database, &self.placements, FreshnessBand::High),
            CompetencyInput::of(&self.authentication, &[], FreshnessBand::Moderate),
            CompetencyInput::of(&self.distributed, refused, FreshnessBand::Unknown),
        ];
        ReadinessView::of(take(&self.bundle, &inputs), &self.competencies())
    }
}

/// Two placements the `distributed` competency refuses, one per basis.
fn refused_placements() -> TestResult<Vec<AxisEvidence>> {
    let entity = support::entity("DISTRIBUTED_FAILURE");
    Ok(vec![
        placed(
            ReadinessAxis::AcademicLearning,
            "reasons-about-partitions",
            "lecture.dist.01",
            &knowledge_record("incident", EvidenceStage::DebuggedIncident, entity)?,
        )?,
        placed(
            ReadinessAxis::DesignChoice,
            "a-criterion-nobody-states",
            "note.dist.01",
            &knowledge_record("exercise", EvidenceStage::Used, entity)?,
        )?,
    ])
}

/// Every stage, so a rubric admits a placement at whichever one a record has.
fn all_stages() -> Vec<EvidenceStage> {
    EvidenceStage::ALL.to_vec()
}

fn fixture() -> TestResult<Fixture> {
    let concept = ontology("RELATIONAL_DATABASE");
    let database = competency_about(DATABASE, &concept, &[INDEXING, MEASURING], &all_stages())?;
    let authentication = competency_about(
        AUTHENTICATION,
        &ontology("AUTHENTICATION"),
        &["selects-a-provider"],
        &all_stages(),
    )?;
    // Two stages only, so a record at any other stage lands in `Unknown` by
    // `RubricAdmitsNoRowAtThatStage` rather than being quietly dropped.
    let distributed = competency_about(
        DISTRIBUTED,
        &ontology("DISTRIBUTED_FAILURE"),
        &["reasons-about-partitions"],
        &[EvidenceStage::Used, EvidenceStage::UnderstoodStructure],
    )?;

    let bundle = bundle(
        "fixture-backend-profile",
        vec![
            entry(&database, BundleImportance::Core),
            entry(&authentication, BundleImportance::Common),
            entry(&distributed, BundleImportance::ContextDependent),
        ],
    )?;

    let entity = support::entity("RELATIONAL_DATABASE");
    let explained = knowledge_record("explained", EvidenceStage::Used, entity)?;
    let exercise = knowledge_record("exercise", EvidenceStage::SolvedProblem, entity)?;
    let incident = knowledge_record("incident", EvidenceStage::DebuggedIncident, entity)?;
    let applied = personal_record("applied", EvidenceStage::MadeDesignChoice)?;

    let placements = vec![
        placed(
            ReadinessAxis::AcademicLearning,
            INDEXING,
            "lecture.db.03",
            &explained,
        )?,
        placed(
            ReadinessAxis::ProblemAndAssignment,
            INDEXING,
            "assignment.index.1",
            &exercise,
        )?,
        placed(
            ReadinessAxis::IncidentDebugging,
            MEASURING,
            "incident.slow-query.1",
            &incident,
        )?,
        // Placed by the user in the project column even though the record is a
        // design-choice one, which is the point: the column is where the user
        // put it and the stage is the record's own.
        placed(
            ReadinessAxis::ProjectApplication,
            MEASURING,
            "project.a.commit.1",
            &applied,
        )?,
    ];

    Ok(Fixture {
        database,
        authentication,
        distributed,
        bundle,
        placements,
    })
}

fn weights(values: [(ReadinessAxis, u32); 5]) -> TestResult<WeightDisclosure> {
    let mut entries = Vec::new();
    for (axis, weight) in values {
        entries.push(AxisWeight::of(axis, weight, "the fixture's own reason")?);
    }
    Ok(WeightDisclosure::of(entries)?)
}

fn even_weights() -> TestResult<WeightDisclosure> {
    weights([
        (ReadinessAxis::AcademicLearning, 1),
        (ReadinessAxis::ProblemAndAssignment, 1),
        (ReadinessAxis::ProjectApplication, 1),
        (ReadinessAxis::IncidentDebugging, 1),
        (ReadinessAxis::DesignChoice, 1),
    ])
}

fn scored(fixture: &Fixture) -> TestResult<ReadinessView> {
    let view = fixture.view()?;
    let competencies = fixture.competencies();
    let matrix = view.matrix();
    Ok(view.publish_score(
        &competencies,
        RubricDisclosure::of(matrix, &competencies)?,
        SourceDisclosure::of(matrix),
        MissingDataDisclosure::of(matrix),
        even_weights()?,
    )?)
}

// ---------------------------------------------------------------------------
// The eight named tests, less the export one.
// ---------------------------------------------------------------------------

/// The matrix is what a reader meets first, and there is no other view.
#[test]
fn matrix_is_the_default_view() -> TestResult {
    let fixture = fixture()?;

    // Every view this crate can produce, over the whole cross-product of the
    // three things that vary: whether a score is published, whether it was
    // then hidden, and whether the weighting was reset.
    let plain = fixture.view()?;
    let published = scored(&fixture)?;
    let hidden = published.hide_score()?;
    let reset = published.reset_weights(
        &fixture.competencies(),
        weights([
            (ReadinessAxis::AcademicLearning, 3),
            (ReadinessAxis::ProblemAndAssignment, 2),
            (ReadinessAxis::ProjectApplication, 5),
            (ReadinessAxis::IncidentDebugging, 4),
            (ReadinessAxis::DesignChoice, 4),
        ])?,
    )?;

    let every: [&ReadinessView; 4] = [&plain, &published, &hidden, &reset];
    for view in every {
        let blocks = view.render();
        assert!(!blocks.is_empty(), "a rendered view has no block at all");
        assert_eq!(
            blocks[0].kind(),
            "MATRIX",
            "block zero of a rendered view is {}",
            blocks[0].kind()
        );
        assert!(
            matches!(blocks[0], ViewBlock::Matrix(_)),
            "block zero is spelled MATRIX and is not the matrix"
        );
    }

    // The matrix is the *default* because it is the only view: the one
    // producer takes no mode, no preference and no flag. This reads the whole
    // set of public functions of the crate that return a `ReadinessView` and
    // requires it to be the four this crate documents, three of which take an
    // existing view.
    let producers = public_signatures_returning("ReadinessView")?;
    assert_eq!(
        producers,
        BTreeSet::from([
            "hide_score".to_owned(),
            "of".to_owned(),
            "publish_score".to_owned(),
            "reset_weights".to_owned(),
        ]),
        "a public function returning a view arrived or left"
    );

    // And the one that opens a view from a matrix takes no mode argument.
    let opener = signature_of("view.rs", "pub fn of(")?;
    assert!(
        opener.contains("matrix: ReadinessMatrix")
            && opener.contains("competencies: &[&Competency]"),
        "ReadinessView::of no longer takes exactly a matrix and its competencies: {opener}"
    );
    Ok(())
}

/// The six columns are the design document's own, in two independent places.
#[test]
fn six_axes_are_separate_columns() -> TestResult {
    let page = design_page()?;

    // Section 24.3's table, in both directions and position by position.
    let table = spec_table_columns(&page)?;
    let declared: Vec<String> = ReadinessAxis::ALL
        .iter()
        .map(|axis| axis.table_heading().to_owned())
        .collect();
    assert_eq!(
        table, declared,
        "section 24.3's matrix columns and ReadinessAxis::ALL disagree"
    );

    // Section 36.9's block, in both directions and position by position.
    let scenario = spec_scenario_keys(&page)?;
    let keys: Vec<String> = ReadinessAxis::ALL
        .iter()
        .map(|axis| axis.scenario_key().to_owned())
        .collect();
    assert_eq!(
        scenario, keys,
        "section 36.9's career view keys and ReadinessAxis::ALL disagree"
    );

    // The two places agree with each other on how many there are, which is what
    // makes six a measurement rather than a number this crate chose.
    assert_eq!(
        table.len(),
        scenario.len(),
        "section 24.3 names {} columns and section 36.9 names {}",
        table.len(),
        scenario.len()
    );

    // Separate means every row carries one cell per axis, no axis twice and
    // none missing, and the freshness column is a different type from the five.
    let fixture = fixture()?;
    let matrix = fixture.matrix();
    for row in matrix.rows() {
        let cells = row.cells();
        assert_eq!(cells.len(), ReadinessAxis::ALL.len());
        let reached: Vec<ReadinessAxis> = cells.iter().map(|(axis, _)| *axis).collect();
        assert_eq!(
            reached,
            ReadinessAxis::ALL.to_vec(),
            "a row's columns are not the six axes in order"
        );
        let unique: BTreeSet<ReadinessAxis> = reached.iter().copied().collect();
        assert_eq!(unique.len(), reached.len(), "a row reaches one axis twice");

        let mut evidence_columns = 0_usize;
        let mut freshness_columns = 0_usize;
        for (axis, reading) in cells {
            match reading {
                academic_readiness::ColumnReading::Evidence(_) => {
                    assert!(!axis.is_freshness());
                    evidence_columns += 1;
                }
                academic_readiness::ColumnReading::Freshness(_) => {
                    assert!(axis.is_freshness());
                    freshness_columns += 1;
                }
            }
        }
        assert_eq!(freshness_columns, 1, "a row has more than one band column");
        assert_eq!(
            evidence_columns + freshness_columns,
            ReadinessAxis::ALL.len()
        );
        assert_eq!(evidence_columns, ReadinessAxis::evidence_axes().len());
    }
    Ok(())
}

/// Section 24.3's second six is `P2-Y1`'s stages, and it is not this one.
#[test]
fn an_axis_and_a_stage_are_two_vocabularies() -> TestResult {
    let page = design_page()?;

    let stages = spec_stage_names(&page)?;
    let declared: Vec<String> = EvidenceStage::ALL
        .iter()
        .map(|stage| stage.spec_name().to_owned())
        .collect();
    assert_eq!(
        stages, declared,
        "section 24.3's evidence stages and P2-Y1's enumeration disagree"
    );

    let columns: BTreeSet<String> = spec_table_columns(&page)?.into_iter().collect();
    let stage_set: BTreeSet<String> = stages.into_iter().collect();
    assert_ne!(
        columns, stage_set,
        "section 24.3's two sixes have become one set, which this crate models as two types"
    );
    // They overlap on exactly one spelling, which is the reason they are two
    // types rather than one. If that changes, the reading recorded in
    // docs/contracts/career-readiness-matrix.md is no longer the document's.
    let shared: Vec<&String> = columns.intersection(&stage_set).collect();
    assert_eq!(
        shared,
        vec![&"설계 선택".to_owned()],
        "the overlap between section 24.3's two sixes moved"
    );

    // And no function in this crate maps one to the other, in either
    // direction. This reads the whole product source rather than a list of
    // names somebody thought of.
    for (file, text) in product_source()? {
        for line in text.lines() {
            let signature = line.trim();
            if !signature.starts_with("pub fn ") && !signature.starts_with("pub const fn ") {
                continue;
            }
            let maps_stage_to_axis = signature.contains("EvidenceStage")
                && signature.contains("ReadinessAxis")
                && signature.contains("->");
            assert!(
                !maps_stage_to_axis,
                "{file} declares a function between a stage and an axis: {signature}"
            );
            let maps_kind_to_axis = signature.contains("EvidenceKind")
                && signature.contains("ReadinessAxis")
                && signature.contains("->");
            assert!(
                !maps_kind_to_axis,
                "{file} declares a function between an evidence kind and an axis: {signature}"
            );
        }
    }
    Ok(())
}

/// No aggregate percentage is the primary output, stated as absences.
#[test]
fn no_primary_aggregate_percentage() -> TestResult {
    let fixture = fixture()?;
    let plain = fixture.view()?;
    let published = scored(&fixture)?;
    let hidden = published.hide_score()?;

    // 1. Position. Block zero is the matrix in every view, and the score --
    //    the one aggregate this crate has -- is never before it.
    let mut emitted: BTreeSet<&'static str> = BTreeSet::new();
    for view in [&plain, &published, &hidden] {
        let blocks = view.render();
        assert_eq!(blocks[0].kind(), "MATRIX");
        for (index, block) in blocks.iter().enumerate() {
            emitted.insert(block.kind());
            if matches!(block, ViewBlock::AuxiliaryScore(_)) {
                assert!(index > 0, "the score was rendered before the matrix");
            }
        }
    }

    // 2. The block vocabulary is closed, in both directions: a kind emitted
    //    that nobody wrote down fails, and a kind written down that nothing
    //    emits fails too.
    let pinned: BTreeSet<&'static str> = ViewBlock::KINDS.into_iter().collect();
    assert_eq!(
        emitted, pinned,
        "the rendered block kinds and ViewBlock::KINDS disagree"
    );

    // 3. The scalar vocabulary is closed. This walks the whole product source
    //    and classifies every declared field's type, so a ratio has no name to
    //    arrive under, whatever it is called.
    let mut score_fields: Vec<String> = Vec::new();
    for (file, text) in product_source()? {
        for line in code_lines(&text) {
            for float in ["f32", "f64"] {
                assert!(
                    !mentions_type(&line, float),
                    "{file} declares an {float}: {line}"
                );
            }
            if let Some(field) = declared_field(&line, "ScoreValue") {
                score_fields.push(format!("{file}::{field}"));
            }
        }
    }
    // The one cross-row aggregate appears as a field in exactly the places that
    // carry it beside its disclosures: the score itself, and the history
    // entries that record what a score said.
    score_fields.sort();
    assert_eq!(
        score_fields,
        vec![
            "history.rs::previous_value".to_owned(),
            "history.rs::value".to_owned(),
            "history.rs::value".to_owned(),
            "score.rs::value".to_owned(),
        ],
        "a ScoreValue field arrived or left"
    );

    // And it is unreachable without the four disclosures: the whole set of
    // public functions returning one is the accessor on `AuxiliaryScore`.
    assert_eq!(
        public_signatures_returning("ScoreValue")?,
        BTreeSet::from(["value".to_owned()]),
        "a public function returning a score value arrived or left"
    );

    // 4. What a reader of a published document sees is closed, in both
    //    directions, at every level. A field of *any* type under *any* name
    //    added to the matrix, a row or the view is a key here -- so a
    //    whole-number standing beside `rows`, which passes the float refusal
    //    above, fails this.
    let document: serde_json::Value = serde_json::to_value(&published)?;
    assert_eq!(
        keys_of(&document)?,
        vec![
            "history".to_owned(),
            "matrix".to_owned(),
            "nonGuaranteeNotice".to_owned(),
            "score".to_owned(),
        ],
        "a published view carries a key nobody wrote down"
    );
    assert_eq!(
        keys_of(&document["matrix"])?,
        vec!["bundle".to_owned(), "rows".to_owned()],
        "a published matrix carries a key nobody wrote down"
    );
    let row = &document["matrix"]["rows"][0];
    assert_eq!(
        keys_of(row)?,
        vec![
            "academic_learning".to_owned(),
            "competency".to_owned(),
            "design_choice".to_owned(),
            "freshness".to_owned(),
            "importance".to_owned(),
            "incident_debugging".to_owned(),
            "problem_and_assignment".to_owned(),
            "project_application".to_owned(),
        ],
        "a published row carries a key nobody wrote down"
    );
    Ok(())
}

/// One JSON object's keys, sorted.
fn keys_of(value: &serde_json::Value) -> TestResult<Vec<String>> {
    let object = value
        .as_object()
        .ok_or("a published value is not an object")?;
    let mut keys: Vec<String> = object.keys().cloned().collect();
    keys.sort();
    Ok(keys)
}

/// A score does not exist without its four disclosures.
#[test]
fn score_without_full_disclosure_is_blocked() -> TestResult {
    let page = design_page()?;
    let sentence = spec_disclosure_sentence(&page)?;
    for word in ["rubric", "source", "누락 데이터", "가중치"] {
        assert!(
            sentence.contains(word),
            "section 24.3 no longer publishes {word}: {sentence}"
        );
    }

    let fixture = fixture()?;
    let view = fixture.view()?;
    let competencies = fixture.competencies();
    let matrix = view.matrix();

    // The one producer takes all four by value and no number.
    let signature = signature_of("score.rs", "pub fn disclose(")?;
    for required in [
        "rubric: RubricDisclosure",
        "sources: SourceDisclosure",
        "missing_data: MissingDataDisclosure",
        "weights: WeightDisclosure",
    ] {
        assert!(
            signature.contains(required),
            "disclose no longer takes {required}: {signature}"
        );
    }
    assert_eq!(
        public_signatures_returning("AuxiliaryScore")?,
        BTreeSet::from(["disclose".to_owned()]),
        "a public function returning a score arrived or left"
    );

    // It publishes when all four are the ones this matrix produces.
    let score = disclose(
        matrix,
        &competencies,
        RubricDisclosure::of(matrix, &competencies)?,
        SourceDisclosure::of(matrix),
        MissingDataDisclosure::of(matrix),
        even_weights()?,
    )?;
    assert!(score.value().weighted_units() > 0);
    assert!(score.value().evidenced_units() < score.value().weighted_units());

    // A disclosure taken of a different matrix is refused, one at a time. The
    // other matrix is the same bundle over one placement instead of four, so
    // every identifier agrees and only the cells differ -- which is what makes
    // the refusal a comparison of the disclosure rather than of the bundle.
    let other = sparse_matrix(&fixture);
    assert_eq!(other.bundle(), matrix.bundle());
    assert_ne!(
        MissingDataDisclosure::of(&other),
        MissingDataDisclosure::of(matrix),
        "the two fixture matrices have the same missing-data disclosure"
    );
    assert_ne!(
        SourceDisclosure::of(&other),
        SourceDisclosure::of(matrix),
        "the two fixture matrices have the same source disclosure"
    );

    let wrong_sources = disclose(
        matrix,
        &competencies,
        RubricDisclosure::of(matrix, &competencies)?,
        SourceDisclosure::of(&other),
        MissingDataDisclosure::of(matrix),
        even_weights()?,
    );
    assert!(
        matches!(
            wrong_sources,
            Err(ReadinessError::DisclosureDoesNotCoverTheMatrix("source", _))
        ),
        "a source disclosure of another matrix was published: {wrong_sources:?}"
    );

    let wrong_missing = disclose(
        matrix,
        &competencies,
        RubricDisclosure::of(matrix, &competencies)?,
        SourceDisclosure::of(matrix),
        MissingDataDisclosure::of(&other),
        even_weights()?,
    );
    assert!(
        matches!(
            wrong_missing,
            Err(ReadinessError::DisclosureDoesNotCoverTheMatrix(
                "missing data",
                _
            ))
        ),
        "a missing-data disclosure of another matrix was published: {wrong_missing:?}"
    );

    // A rubric disclosure cannot even be taken when a row's competency is
    // absent, so a score over rows nobody can drill into has no first argument.
    let short = RubricDisclosure::of(matrix, &[&fixture.database]);
    assert!(
        matches!(
            short,
            Err(ReadinessError::DisclosureDoesNotCoverTheMatrix("rubric", _))
        ),
        "a rubric disclosure was taken over rows it does not cover: {short:?}"
    );

    // A weighting that leaves a column out is refused, one column at a time.
    for skipped in ReadinessAxis::evidence_axes() {
        let mut entries = Vec::new();
        for axis in ReadinessAxis::evidence_axes() {
            if axis != skipped {
                entries.push(AxisWeight::of(axis, 1, "the fixture's own reason")?);
            }
        }
        assert!(
            matches!(
                WeightDisclosure::of(entries),
                Err(ReadinessError::WeightingIsNotTotal)
            ),
            "a weighting without {} was admitted",
            skipped.as_str()
        );
    }

    // And the freshness column is not weightable at all, because it is not
    // evidence.
    assert!(matches!(
        AxisWeight::of(ReadinessAxis::Freshness, 1, "because"),
        Err(ReadinessError::FreshnessIsNotAnEvidenceColumn)
    ));
    assert!(matches!(
        AxisWeight::of(ReadinessAxis::DesignChoice, 1, "   "),
        Err(ReadinessError::EmptyText("weight reason"))
    ));
    Ok(())
}

/// Missing, unknown and freshness are three readings, not one axis.
#[test]
fn missing_and_unknown_are_separate_from_freshness() -> TestResult {
    let page = design_page()?;
    let row = spec_career_failure_row(&page)?;
    let display = row
        .last()
        .ok_or("section 34.5's career row has no display cell")?;
    assert_eq!(
        display, "missing/unknown과 freshness를 별도 표시",
        "section 34.5's career readiness row no longer separates the three"
    );

    let fixture = fixture()?;

    // A record whose stage the rubric does not admit for the criterion it
    // names, and one that names a criterion the competency does not state.
    let refused = refused_placements()?;
    let inputs = vec![
        CompetencyInput::of(&fixture.database, &fixture.placements, FreshnessBand::High),
        CompetencyInput::of(&fixture.authentication, &[], FreshnessBand::Moderate),
        // The band is `Unknown` and two of its columns are `Unknown` for two
        // different reasons, which is exactly the case that would collapse if
        // the three were one axis.
        CompetencyInput::of(&fixture.distributed, &refused, FreshnessBand::Unknown),
    ];
    let matrix = take(&fixture.bundle, &inputs);
    let row = matrix
        .row(fixture.distributed.id())
        .ok_or("the bundle lost its third row")?;

    assert_eq!(row.freshness().band(), FreshnessBand::Unknown);
    let academic = row
        .evidence_cell(ReadinessAxis::AcademicLearning)
        .ok_or("the academic column is not an evidence column")?;
    assert_eq!(academic.reading(), "UNKNOWN");
    assert_eq!(
        academic
            .refused()
            .iter()
            .map(academic_readiness::RefusedPlacement::basis)
            .collect::<Vec<_>>(),
        vec![UnknownBasis::RubricAdmitsNoRowAtThatStage]
    );
    let design = row
        .evidence_cell(ReadinessAxis::DesignChoice)
        .ok_or("the design column is not an evidence column")?;
    assert_eq!(design.reading(), "UNKNOWN");
    assert_eq!(
        design
            .refused()
            .iter()
            .map(academic_readiness::RefusedPlacement::basis)
            .collect::<Vec<_>>(),
        vec![UnknownBasis::NamesNoStatedCriterion]
    );
    let project = row
        .evidence_cell(ReadinessAxis::ProjectApplication)
        .ok_or("the project column is not an evidence column")?;
    assert_eq!(project.reading(), "MISSING");

    // Three readings, and the band is not one of them.
    let readings: BTreeSet<&'static str> = ReadinessAxis::ALL
        .into_iter()
        .filter_map(|axis| row.evidence_cell(axis))
        .map(AxisCell::reading)
        .collect();
    assert!(readings.is_subset(&AxisCell::READINGS.into_iter().collect()));
    assert_eq!(
        readings,
        BTreeSet::from(["MISSING", "UNKNOWN"]),
        "the all-unknown row's readings moved"
    );

    // A missing cell and an unknown cell are separately disclosed, and the
    // band is disclosed as neither.
    let disclosure = MissingDataDisclosure::of(&matrix);
    assert!(disclosure.missing_count() > 0);
    assert!(disclosure.unknown_count() > 0);
    assert_eq!(
        disclosure.entries().len(),
        disclosure.missing_count() + disclosure.unknown_count()
    );
    for entry in disclosure.entries() {
        assert!(
            !entry.axis().is_freshness(),
            "the freshness column was listed as missing data"
        );
    }

    // The band and the cell are two types with no conversion in either
    // direction, over the whole product source.
    for (file, text) in product_source()? {
        for line in code_lines(&text) {
            // A declaration binds a type to a name or returns one. A
            // re-export list does neither, and its continuation lines carry
            // bare identifiers that would otherwise read as one.
            if !line.contains(": ") && !line.contains("->") {
                continue;
            }
            let converts = (line.contains("FreshnessBand") && line.contains("AxisCell"))
                || (line.contains("FreshnessCell") && line.contains("AxisCell"));
            assert!(
                !converts,
                "{file} names a band and a cell in one declaration: {line}"
            );
        }
    }
    Ok(())
}

/// Every path of every direction ends at a criterion or an explicit absence.
#[test]
fn four_navigation_directions_terminate_at_criterion_and_evidence() -> TestResult {
    let page = design_page()?;
    let bullets = spec_direction_bullets(&page)?;
    let declared: Vec<String> = NavigationDirection::ALL
        .iter()
        .map(|direction| direction.specification_bullet().to_owned())
        .collect();
    assert_eq!(
        bullets, declared,
        "section 24.4's bullets and NavigationDirection::ALL disagree"
    );

    let fixture = fixture()?;
    let claim = personal_claim()?;

    // Two views: one whose third row nobody supplied, and one whose third row
    // carries two placements it refuses. Between them every cell reading a walk
    // can meet is present, which is what makes the sweep below exhaustive over
    // what a terminus can be rather than over what this fixture happened to
    // hold.
    let views = [
        fixture.view()?,
        fixture.view_with_refusals(&refused_placements()?)?,
    ];

    // Every direction, from a start that reaches something and from one that
    // reaches nothing. Exhaustive over the four directions and the two cases.
    let reaching: Vec<StartingPoint> = vec![
        StartingPoint::Concept(ontology("RELATIONAL_DATABASE")),
        StartingPoint::GoalOrRole(fixture.bundle.reference()),
        StartingPoint::Project(StartingPointId::new(claim.id().as_str())?),
        StartingPoint::Course(StartingPointId::new(knowledge_evidence_id("explained"))?),
    ];
    let missing: Vec<StartingPoint> = vec![
        StartingPoint::Concept(ontology("A_CONCEPT_NOBODY_RECORDED")),
        StartingPoint::GoalOrRole(fixture.bundle.reference()),
        StartingPoint::Project(StartingPointId::new("a-claim-nobody-promoted")?),
        StartingPoint::Course(StartingPointId::new("an-item-nobody-admitted")?),
    ];

    let mut directions_walked: BTreeSet<NavigationDirection> = BTreeSet::new();
    let mut kinds: BTreeSet<&'static str> = BTreeSet::new();
    let mut absences: BTreeSet<&'static str> = BTreeSet::new();

    for (label, starts) in [("reaching", &reaching), ("missing", &missing)] {
        for start in starts {
            for view in &views {
                let walk = traverse(view, start);
                directions_walked.insert(walk.direction());
                assert_eq!(walk.direction(), start.direction());
                let termini = walk.termini();
                assert!(
                    !termini.is_empty(),
                    "the {label} walk in {} ended nowhere",
                    walk.direction().as_str()
                );
                for terminus in termini {
                    kinds.insert(terminus.kind());
                    match terminus {
                        Terminus::CriterionAndEvidence {
                            criterion, locator, ..
                        } => {
                            assert!(!criterion.as_str().is_empty());
                            assert!(!locator.as_str().is_empty());
                            absences.insert("NONE");
                        }
                        Terminus::ExplicitAbsence(state) => {
                            let named = match state {
                                AbsenceState::CellIsMissing { criterion, .. } => {
                                    assert!(!criterion.as_str().is_empty());
                                    "CELL_IS_MISSING"
                                }
                                AbsenceState::CellIsUnknown { criterion, .. } => {
                                    assert!(!criterion.as_str().is_empty());
                                    "CELL_IS_UNKNOWN"
                                }
                                AbsenceState::NoRowReachesTheStartingPoint {
                                    direction, ..
                                } => {
                                    assert_eq!(*direction, walk.direction());
                                    "NO_ROW_REACHES_THE_STARTING_POINT"
                                }
                            };
                            absences.insert(named);
                        }
                    }
                }
            }
        }
    }

    assert_eq!(
        directions_walked,
        NavigationDirection::ALL.into_iter().collect(),
        "a direction was never walked"
    );
    assert_eq!(
        kinds,
        Terminus::KINDS.into_iter().collect(),
        "the terminus kinds emitted and Terminus::KINDS disagree"
    );
    assert_eq!(
        absences,
        BTreeSet::from([
            "CELL_IS_MISSING",
            "CELL_IS_UNKNOWN",
            "NONE",
            "NO_ROW_REACHES_THE_STARTING_POINT",
        ]),
        "an absence state was never reached, so its arm is untested"
    );

    // A walk that reaches nothing still names a criterion or the start; a walk
    // that reaches something reaches a locator somebody can open.
    let reaching_concept = traverse(&views[0], &reaching[0]);
    assert!(
        reaching_concept
            .termini()
            .iter()
            .any(|terminus| matches!(terminus, Terminus::CriterionAndEvidence { .. })),
        "the concept walk found no evidence at all"
    );
    let missing_project = traverse(&views[0], &missing[2]);
    assert_eq!(missing_project.termini().len(), 1);
    assert!(matches!(
        missing_project.termini()[0],
        Terminus::ExplicitAbsence(AbsenceState::NoRowReachesTheStartingPoint { .. })
    ));
    Ok(())
}

/// Hiding a score and resetting the weighting keep what they replaced.
#[test]
fn score_hide_and_weight_reset_preserve_history() -> TestResult {
    let page = design_page()?;
    let row = spec_career_failure_row(&page)?;
    let recovery = row
        .get(row.len() - 2)
        .ok_or("section 34.5's career row has no recovery cell")?;
    assert!(
        recovery.contains("score 숨김") && recovery.contains("가중치 초기화"),
        "section 34.5's career readiness recovery moved: {recovery}"
    );

    let fixture = fixture()?;
    let competencies = fixture.competencies();
    let published = scored(&fixture)?;
    let published_bytes = serde_json::to_string(&published)?;

    let hidden = published.hide_score()?;
    let heavier = weights([
        (ReadinessAxis::AcademicLearning, 1),
        (ReadinessAxis::ProblemAndAssignment, 1),
        (ReadinessAxis::ProjectApplication, 7),
        (ReadinessAxis::IncidentDebugging, 1),
        (ReadinessAxis::DesignChoice, 1),
    ])?;
    let reweighted = published.reset_weights(&competencies, heavier)?;

    // Neither touched its base, byte for byte.
    assert_eq!(
        serde_json::to_string(&published)?,
        published_bytes,
        "publishing a change edited the view it was taken of"
    );

    // Both histories extend the base's, and neither shortens it.
    for later in [&hidden, &reweighted] {
        assert!(
            later.history().len() > published.history().len(),
            "a recovery did not record itself"
        );
        assert_eq!(
            &later.history()[..published.history().len()],
            published.history(),
            "a recovery rewrote what was already recorded"
        );
    }

    // The hidden score is gone from the display and kept in the history.
    assert!(hidden.score().is_none());
    assert!(
        !hidden
            .render()
            .iter()
            .any(|block| matches!(block, ViewBlock::AuxiliaryScore(_))),
        "a hidden score is still rendered"
    );
    let hidden_value = published
        .score()
        .ok_or("the published view displays no score")?
        .value();
    assert!(
        hidden.history().iter().any(|event| matches!(
            event,
            ReadinessEvent::ScoreHidden { value, .. } if *value == hidden_value
        )),
        "the hidden number is not in the history"
    );

    // The reset kept both weightings and the number the old one produced.
    let reset_entry = reweighted
        .history()
        .last()
        .ok_or("the reweighted view has no history")?;
    let ReadinessEvent::WeightsReset {
        from,
        to,
        previous_value,
    } = reset_entry
    else {
        return Err(format!("the last event is {}", reset_entry.as_str()).into());
    };
    assert_eq!(from, &even_weights()?);
    assert_ne!(from, to);
    assert_eq!(*previous_value, hidden_value);
    assert_ne!(
        reweighted
            .score()
            .ok_or("the reweighted view displays no score")?
            .value(),
        *previous_value,
        "the reset produced the same number under a different weighting"
    );

    // Nothing in this crate takes `&mut self`, which is what makes the two
    // above properties of the program rather than of these two functions.
    for (file, text) in product_source()? {
        for line in code_lines(&text) {
            assert!(
                !line.contains("&mut self"),
                "{file} takes &mut self, so a view could be edited in place: {line}"
            );
        }
    }

    // And there is no arm meaning the score was deleted.
    assert_eq!(
        ReadinessEvent::KINDS.len(),
        3,
        "the history vocabulary changed without this test being told"
    );
    Ok(())
}

/// A row is of a bundle at an exact version, never of a rendered name.
#[test]
fn the_matrix_is_of_a_bundle_at_an_exact_version() -> TestResult {
    let fixture = fixture()?;
    let matrix = fixture.matrix();
    assert_eq!(matrix.bundle(), &fixture.bundle.reference());
    assert_eq!(matrix.bundle().version(), fixture.bundle.version());

    // The row set is the bundle's own membership, in the bundle's own order,
    // including the entry nobody supplied an input for.
    let rows: Vec<String> = matrix
        .rows()
        .iter()
        .map(|row| row.competency().as_str().to_owned())
        .collect();
    let entries: Vec<String> = fixture
        .bundle
        .competencies()
        .iter()
        .map(|entry| entry.competency().as_str().to_owned())
        .collect();
    assert_eq!(rows, entries);
    assert_eq!(
        matrix
            .row(fixture.distributed.id())
            .ok_or("the unsupplied competency has no row")?
            .freshness()
            .band(),
        FreshnessBand::Unknown,
        "an unsupplied competency was given a band"
    );
    for axis in ReadinessAxis::evidence_axes() {
        assert_eq!(
            matrix
                .row(fixture.distributed.id())
                .and_then(|row| row.evidence_cell(axis))
                .map(AxisCell::reading),
            Some("MISSING"),
            "an unsupplied competency's {} column is not missing",
            axis.as_str()
        );
    }

    // An input naming a competency the bundle does not list reaches no row.
    let stranger = competency_about(
        "not-in-the-bundle",
        &ontology("SOMETHING_ELSE"),
        &["a-criterion"],
        &[EvidenceStage::Used],
    )?;
    let stray = take(
        &fixture.bundle,
        &[CompetencyInput::of(&stranger, &[], FreshnessBand::VeryHigh)],
    );
    assert_eq!(stray.rows().len(), fixture.bundle.competencies().len());
    assert!(stray.row(stranger.id()).is_none());

    // A view whose rows it was given no competency for is refused, so no walk
    // can reach a row it cannot drill into.
    assert!(matches!(
        ReadinessView::of(fixture.matrix(), &[&fixture.database]),
        Err(ReadinessError::DisclosureDoesNotCoverTheMatrix(
            "criteria",
            _
        ))
    ));
    Ok(())
}

/// A walk over a matrix cannot run zero times, and neither refusal is this
/// crate's.
#[test]
fn a_walk_over_a_matrix_cannot_run_zero_times() -> TestResult {
    // `P2-Y2` refuses a bundle that names no competency, so a matrix has a row.
    let empty = bundle("fixture-empty-profile", Vec::new());
    assert!(
        empty.is_err(),
        "P2-Y2 admitted a bundle with no competency, so a matrix can have no row"
    );

    // `P2-Y1` refuses a competency that states no criterion, so a row has a
    // criterion to end a walk at.
    let none = academic_competency::declare(
        academic_competency::CompetencyId::new("states-nothing")?,
        academic_competency::Situation::new("a fixture situation")?,
        Vec::new(),
        vec![academic_competency::EnablingConcept::of(
            ontology("SOMETHING"),
            academic_competency::ContributionImportance::Substantial,
            academic_competency::Necessity::Necessary,
        )],
        academic_competency::EvidenceRubric::of(Vec::new()),
    );
    assert!(
        none.is_err(),
        "P2-Y1 admitted a competency with no criterion, so a walk could end nowhere"
    );

    // And this crate adds no third check of its own, so there is no branch here
    // guarding a case its own inputs cannot produce.
    let fixture = fixture()?;
    let view = fixture.view()?;
    for row in view.matrix().rows() {
        assert!(
            !view.criteria_of(row.competency()).is_empty(),
            "a row of a view has no criterion"
        );
    }
    Ok(())
}

/// The identifier rule is executed over every byte, not searched for.
#[test]
fn an_identifier_is_classified_byte_by_byte() -> TestResult {
    assert!(EvidenceLocatorId::new("lecture.db-03_1").is_ok());
    assert!(EvidenceLocatorId::new("a".repeat(64)).is_ok());
    for refused in [
        String::new(),
        "a".repeat(65),
        "lecture 03".to_owned(),
        "lecture/03".to_owned(),
        "lecture\u{0}03".to_owned(),
        "강의".to_owned(),
        "lecture\n03".to_owned(),
        "lecture+03".to_owned(),
    ] {
        assert!(
            EvidenceLocatorId::new(refused.clone()).is_err(),
            "{refused:?} was admitted as an evidence locator"
        );
        assert!(StartingPointId::new(refused).is_err());
    }

    // Every byte outside the class is refused, not only the ones somebody
    // listed. This walks the whole single-byte range in both directions.
    for byte in 0_u8..=255 {
        let value = String::from_utf8_lossy(&[byte]).into_owned();
        let admitted = EvidenceLocatorId::new(value.clone()).is_ok();
        let legal = byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-');
        assert_eq!(
            admitted,
            legal,
            "byte {byte} was {} and the rule says {}",
            if admitted { "admitted" } else { "refused" },
            if legal { "legal" } else { "illegal" }
        );
        assert_eq!(StartingPointId::new(value).is_ok(), legal);
    }
    Ok(())
}

/// The notice is the design document's own words, in both places.
#[test]
fn the_notice_is_the_specifications_own_words() -> TestResult {
    let page = design_page()?;
    let sentence = spec_disclosure_sentence(&page)?;
    assert!(
        sentence.contains(academic_readiness::SPECIFICATION_PHRASE),
        "section 24.3 no longer says {}",
        academic_readiness::SPECIFICATION_PHRASE
    );

    let row = spec_anti_goal_row(&page)?;
    assert_eq!(row.len(), 3, "section 35's row is not three cells: {row:?}");
    assert_eq!(row[0], academic_readiness::REFUSED_PRODUCT);
    assert_eq!(row[1], academic_readiness::REFUSAL_REASON);
    assert_eq!(row[2], academic_readiness::ALLOWED_INSTEAD);

    let text = NonGuaranteeNotice::rendered().text();
    for span in [
        academic_readiness::SPECIFICATION_PHRASE,
        academic_readiness::REFUSED_PRODUCT,
        academic_readiness::REFUSAL_REASON,
        academic_readiness::ALLOWED_INSTEAD,
    ] {
        assert!(text.contains(span), "the notice dropped {span}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers that read this crate's own source.
// ---------------------------------------------------------------------------

/// Every `.rs` file of this crate's product source, with its text.
fn product_source() -> TestResult<Vec<(String, String)>> {
    let root = support::workspace_root().join("crates/readiness/src");
    let mut files = Vec::new();
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or("a source file has no name")?
                .to_owned();
            files.push((name, fs::read_to_string(&path)?));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        files.len(),
        10,
        "the product source has {} modules, and the walk was written for ten",
        files.len()
    );
    Ok(files)
}

/// Every line of one source file that is not a comment, trimmed.
fn code_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.starts_with('*'))
        .map(str::to_owned)
        .collect()
}

/// Whether `name` appears as a whole identifier on this code line.
fn mentions_type(line: &str, name: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find(name) {
        let before = rest[..at].chars().next_back();
        let after = rest[at + name.len()..].chars().next();
        if before.is_none_or(|value| !value.is_alphanumeric() && value != '_')
            && after.is_none_or(|value| !value.is_alphanumeric() && value != '_')
        {
            return true;
        }
        rest = &rest[at + name.len()..];
    }
    false
}

/// The field name, when this code line declares a struct or variant field of
/// `name`'s type.
///
/// A field declaration and nothing else: `value: ScoreValue,` is one, and
/// `-> ScoreValue` and `value: ScoreValue {` are not.
fn declared_field(line: &str, name: &str) -> Option<String> {
    let body = line.strip_suffix(',')?;
    let (field, declared) = body.split_once(": ")?;
    if declared.trim() != name {
        return None;
    }
    let field = field.trim_start_matches("pub ").trim();
    field
        .chars()
        .all(|value| value.is_ascii_lowercase() || value == '_')
        .then(|| field.to_owned())
}

/// The whole set of public function names in this crate returning `name`.
///
/// `Self` is resolved to the type of the enclosing `impl` block, so a
/// constructor written `-> Result<Self, ReadinessError>` is found rather than
/// missed. A scanner that missed it would report an empty set, which is the
/// third of `docs/contracts/policy-source-scans.md`'s three empty shapes, so
/// every caller compares against a non-empty expectation and
/// `the_signature_scanner_is_not_vacuous` exercises it in both directions.
fn public_signatures_returning(name: &str) -> TestResult<BTreeSet<String>> {
    let mut found = BTreeSet::new();
    for (_, text) in product_source()? {
        found.extend(signatures_returning(&text, name));
    }
    Ok(found)
}

/// The same walk over one file's text.
fn signatures_returning(text: &str, name: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut enclosing = String::new();
    let mut held: Option<String> = None;
    for line in code_lines(text) {
        if let Some(rest) = line.strip_prefix("impl ") {
            let head = rest.trim_end_matches(" {").trim();
            let subject = head.split_once(" for ").map_or(head, |(_, after)| after);
            enclosing = subject
                .split(['<', ' '])
                .next()
                .unwrap_or(subject)
                .to_owned();
        }
        let line = line.replace("pub const fn ", "pub fn ");
        let mut signature = held.take().map_or_else(String::new, |value| value + " ");
        signature.push_str(&line);
        if !signature.contains("pub fn ") {
            continue;
        }
        // A signature is complete once its parameter list has closed.
        if signature.matches('(').count() > signature.matches(')').count() {
            held = Some(signature);
            continue;
        }
        let Some(at) = signature.find("pub fn ") else {
            continue;
        };
        let after = &signature[at + "pub fn ".len()..];
        let Some(open) = after.find('(') else {
            continue;
        };
        let function = after[..open].trim().to_owned();
        let body = after.find(" {").unwrap_or(after.len());
        let resolved = after[..body].replace("Self", &enclosing);
        if returns(&resolved, name) {
            found.insert(function);
        }
    }
    found
}

/// Whether one flattened signature *produces* a `name`.
///
/// A produced value is returned by value, in any of the shapes this crate
/// uses: bare, optional, fallible and vectored. A **borrowed** return is an
/// accessor on a value somebody already holds and is deliberately not a
/// producer, which is what lets `the whole set of producers is one` be a claim
/// about how a value comes into existence rather than about how it is read.
fn returns(signature: &str, name: &str) -> bool {
    let Some(arrow) = signature.find("->") else {
        return false;
    };
    let returned = &signature[arrow + 2..];
    let mut rest = returned;
    let mut offset = 0_usize;
    while let Some(at) = rest.find(name) {
        let absolute = offset + at;
        let before = returned[..absolute].chars().next_back();
        let after = returned[absolute + name.len()..].chars().next();
        let bounded = before.is_none_or(|value| !value.is_alphanumeric() && value != '_')
            && after.is_none_or(|value| !value.is_alphanumeric() && value != '_');
        if bounded && before != Some('&') {
            return true;
        }
        offset = absolute + name.len();
        rest = &returned[offset..];
    }
    false
}

/// One declaration's whole text, from one module of the product source.
///
/// The module is named because several modules declare a `pub fn of(`, and a
/// scan that took the first would measure whichever file sorted first.
fn signature_of(module: &str, opening: &str) -> TestResult<String> {
    for (name, text) in product_source()? {
        if name != module {
            continue;
        }
        if let Some(at) = text.find(opening) {
            let rest = &text[at..];
            let end = rest.find(" {").ok_or("a declaration has no body")?;
            return Ok(rest[..end]
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" "));
        }
    }
    Err(format!("no declaration in {module} opens with {opening}").into())
}

/// The signature scanner finds what it is pointed at and refuses what it is
/// not.
///
/// `P2-P1` measured a guard that read the wrong thing and passed, and `P2-Y2`
/// measured a scanner whose narrow pattern silently found nothing. This is the
/// control: the scanner is run over a fragment whose answers are known, in
/// both directions, including the `-> Result<Self, _>` shape that a naive
/// scanner misses.
#[test]
fn the_signature_scanner_is_not_vacuous() -> TestResult {
    const FRAGMENT: &str = "\
impl ReadinessView {
    /// pub fn commented(&self) -> ReadinessView
    pub fn of(
        matrix: ReadinessMatrix,
        competencies: &[&Competency],
    ) -> Result<Self, ReadinessError> {
    pub fn matrix(&self) -> &ReadinessMatrix {
    pub const fn band(&self) -> FreshnessCell {
    fn private(&self) -> Self {
}
impl Serialize for ReadinessMatrix {
    pub fn take(bundle: &RoleProfile) -> ReadinessMatrix {
}
";
    assert_eq!(
        signatures_returning(FRAGMENT, "ReadinessView"),
        BTreeSet::from(["of".to_owned()]),
        "the scanner missed a -> Result<Self, _> constructor or found a comment or a private fn"
    );
    assert_eq!(
        signatures_returning(FRAGMENT, "ReadinessMatrix"),
        BTreeSet::from(["take".to_owned()]),
        "the scanner counted a borrowed accessor as a producer, or missed a bare return"
    );
    assert_eq!(
        signatures_returning(FRAGMENT, "FreshnessCell"),
        BTreeSet::from(["band".to_owned()]),
        "the scanner missed a `pub const fn` producer"
    );
    assert_eq!(
        signatures_returning(FRAGMENT, "ReadinessMatrixEntry"),
        BTreeSet::new(),
        "the scanner matched a name that is only a prefix of the declared one"
    );

    // The two line classifiers, in both directions.
    assert_eq!(code_lines("// a\nlet x = 1;\n * b\n"), vec!["let x = 1;"]);
    assert!(mentions_type("value: ScoreValue,", "ScoreValue"));
    assert!(!mentions_type("value: ScoreValues,", "ScoreValue"));
    assert_eq!(
        declared_field("value: ScoreValue,", "ScoreValue"),
        Some("value".to_owned())
    );
    assert_eq!(declared_field("-> ScoreValue {", "ScoreValue"), None);
    assert_eq!(declared_field("value: ScoreValue {", "ScoreValue"), None);
    assert_eq!(declared_field("value: OtherValue,", "ScoreValue"), None);
    Ok(())
}

/// The same bundle with one placement instead of four, so its locator set and
/// its missing-data set are both different from the full matrix's.
fn sparse_matrix(fixture: &Fixture) -> ReadinessMatrix {
    let one = &fixture.placements[..1];
    take(
        &fixture.bundle,
        &[
            CompetencyInput::of(&fixture.database, one, FreshnessBand::High),
            CompetencyInput::of(&fixture.authentication, &[], FreshnessBand::Moderate),
        ],
    )
}

/// The score type is never reachable except through its disclosures.
#[test]
fn the_only_score_producer_is_disclose() -> TestResult {
    let fixture = fixture()?;
    let view = scored(&fixture)?;
    let score: &AuxiliaryScore = view.score().ok_or("the scored view displays no score")?;
    assert_eq!(
        score.weights().weights().len(),
        ReadinessAxis::evidence_axes().len()
    );
    assert!(!score.rubric().entries().is_empty());
    assert_eq!(
        score.rubric().entries().len(),
        view.matrix().rows().len(),
        "the rubric disclosure does not cover every row"
    );
    let value: ScoreValue = score.value();
    assert!(value.weighted_units() >= value.evidenced_units());
    Ok(())
}
