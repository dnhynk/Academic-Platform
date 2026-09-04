//! Section 24.3's cells: which criterion is settled, at which stage, by what.
//!
//! ## What fills a cell, in full
//!
//! A [`StageEvidence`] settles the cell at (`criterion`, `stage`) when **both**
//! of the following hold, and there is no third case and no weaker one:
//!
//! 1. the competency's rubric declares a row at that criterion and that stage —
//!    the author said this stage witnesses this criterion; and
//! 2. the criterion **names** the concept the record is about, whole-pair,
//!    namespace included.
//!
//! There is deliberately **no** arm that reads the competency's
//! `enabledByConcepts` when a criterion names no concept, because
//! [`PerformanceCriterion::of`][c] refuses a criterion that names none. That
//! arm is section 24.3's own counter-example one level up: a competency six
//! concepts enable would otherwise have every cell settled by evidence about
//! any one of them.
//!
//! [c]: crate::PerformanceCriterion::of
//!
//! A record that settles no cell is not discarded quietly — it is in
//! [`RubricSheet::unmatched`], so a caller can see that evidence arrived and
//! filled nothing rather than inferring it from a cell that stayed empty.
//!
//! ## The sheet is a derivation and deserializes into nothing
//!
//! [`fill`] is a pure function of a competency and a slice of records, and
//! [`RubricSheet`] is `Serialize` and not `Deserialize` for the reason
//! [`crate::evidence`] gives: reading one back would be a way to have a filled
//! cell that ran neither producer.

use serde::Serialize;

use crate::{
    Competency,
    evidence::StageEvidence,
    identity::{CompetencyId, CriterionId},
    stage::EvidenceStage,
};

/// What one cell of section 24.3's matrix says.
///
/// Three readings, and the last two are not the same thing: a cell the rubric
/// admits and nothing has settled is a gap in the evidence, and a cell the
/// rubric does not admit is a stage this criterion was never going to be
/// witnessed at. Section 24.3's example table writes both as `—`; separating
/// them is what lets `P2-Y3` display them apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    content = "records",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum CellState {
    /// The rubric admits this stage here and these records settle it.
    Filled(Vec<StageEvidence>),
    /// The rubric admits this stage here and nothing settles it.
    Empty,
    /// The rubric declares no row here.
    NotInRubric,
}

impl CellState {
    /// The records settling this cell, which is empty unless it is filled.
    #[must_use]
    pub fn records(&self) -> &[StageEvidence] {
        match self {
            Self::Filled(records) => records,
            Self::Empty | Self::NotInRubric => &[],
        }
    }

    /// Whether anything settles it.
    #[must_use]
    pub const fn is_filled(&self) -> bool {
        matches!(self, Self::Filled(_))
    }
}

/// One cell: one criterion, at one stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RubricCell {
    criterion: CriterionId,
    stage: EvidenceStage,
    /// The rubric row's own text, when there is a row.
    admits: Option<String>,
    state: CellState,
}

impl RubricCell {
    /// Which criterion.
    #[must_use]
    pub const fn criterion(&self) -> &CriterionId {
        &self.criterion
    }

    /// At which of section 24.3's stages.
    #[must_use]
    pub const fn stage(&self) -> EvidenceStage {
        self.stage
    }

    /// What the rubric said a reader has to be able to open here.
    #[must_use]
    pub fn admits(&self) -> Option<&str> {
        self.admits.as_deref()
    }

    /// What the cell says.
    #[must_use]
    pub const fn state(&self) -> &CellState {
        &self.state
    }
}

/// One competency's cells, and the records that settled none of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RubricSheet {
    competency: CompetencyId,
    cells: Vec<RubricCell>,
    unmatched: Vec<StageEvidence>,
}

impl RubricSheet {
    /// Which competency this sheet is about.
    #[must_use]
    pub const fn competency(&self) -> &CompetencyId {
        &self.competency
    }

    /// Every cell: one per criterion per stage, in criterion order then section
    /// 24.3's stage order.
    #[must_use]
    pub fn cells(&self) -> &[RubricCell] {
        &self.cells
    }

    /// One cell.
    #[must_use]
    pub fn cell(&self, criterion: &CriterionId, stage: EvidenceStage) -> Option<&RubricCell> {
        self.cells
            .iter()
            .find(|cell| cell.criterion() == criterion && cell.stage() == stage)
    }

    /// The records that settled no cell.
    ///
    /// Evidence about a concept no criterion names, or at a stage no rubric row
    /// admits, arrives here rather than nowhere.
    #[must_use]
    pub fn unmatched(&self) -> &[StageEvidence] {
        &self.unmatched
    }

    /// Every filled cell.
    #[must_use]
    pub fn filled(&self) -> Vec<&RubricCell> {
        self.cells
            .iter()
            .filter(|cell| cell.state().is_filled())
            .collect()
    }
}

/// Settles `competency`'s cells with `records`.
///
/// A pure function of its two arguments. It reads no clock, opens nothing, and
/// takes no `&mut`: a correction is a new call over new records.
#[must_use]
pub fn fill(competency: &Competency, records: &[StageEvidence]) -> RubricSheet {
    let mut cells = Vec::new();
    let mut matched = vec![false; records.len()];

    for criterion in competency.criteria() {
        for stage in EvidenceStage::ALL {
            let Some(row) = competency.rubric().row(criterion.id(), stage) else {
                cells.push(RubricCell {
                    criterion: criterion.id().clone(),
                    stage,
                    admits: None,
                    state: CellState::NotInRubric,
                });
                continue;
            };
            let mut settling = Vec::new();
            for (index, record) in records.iter().enumerate() {
                // Both halves, and nothing else. The stage is the record's own
                // and the concept is read out of the record's foundation, so
                // neither side of this comparison is a caller's assertion.
                if record.stage() == stage && criterion.is_about(record.concept()) {
                    matched[index] = true;
                    settling.push(record.clone());
                }
            }
            let state = if settling.is_empty() {
                CellState::Empty
            } else {
                CellState::Filled(settling)
            };
            cells.push(RubricCell {
                criterion: criterion.id().clone(),
                stage,
                admits: Some(row.admits().to_owned()),
                state,
            });
        }
    }

    let unmatched = records
        .iter()
        .enumerate()
        .filter(|(index, _)| !matched[*index])
        .map(|(_, record)| record.clone())
        .collect();

    RubricSheet {
        competency: competency.id().clone(),
        cells,
        unmatched,
    }
}
