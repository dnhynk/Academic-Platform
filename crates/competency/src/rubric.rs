//! Section 24.1's `evidenceRubric`, as rows a reader can open.
//!
//! The design's own example is three rows — `trace analysis`, `incident
//! diagnosis`, `written explanation with measurements` — and each of them names
//! what somebody would look at. A row here carries that text, the criterion it
//! settles, and the section 24.3 stage at which it settles it.
//!
//! ## The rubric is what makes a competency observable
//!
//! A criterion no row witnesses is a criterion nobody can check, and a
//! competency whose criteria nobody can check is the `knows X` statement
//! section 7.1 refuses. [`crate::declare`] therefore requires every criterion
//! to be named by at least one row and every row to name a criterion the
//! competency has, in both directions. Neither hole has a representation that
//! survives the constructor.

use serde::{Deserialize, Serialize};

use crate::{
    CompetencyError,
    identity::{CriterionId, non_empty},
    stage::EvidenceStage,
};

/// One row of section 24.1's `evidenceRubric`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RubricRowWire", into = "RubricRowWire")]
pub struct RubricRow {
    criterion: CriterionId,
    stage: EvidenceStage,
    admits: String,
}

impl RubricRow {
    /// Records one row.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::EmptyText`] when the row says nothing about what a
    /// reader would open.
    pub fn of(
        criterion: CriterionId,
        stage: EvidenceStage,
        admits: impl Into<String>,
    ) -> Result<Self, CompetencyError> {
        Ok(Self {
            criterion,
            stage,
            admits: non_empty(admits.into(), "evidence rubric")?,
        })
    }

    /// Which criterion this row settles.
    #[must_use]
    pub const fn criterion(&self) -> &CriterionId {
        &self.criterion
    }

    /// At which of section 24.3's stages.
    #[must_use]
    pub const fn stage(&self) -> EvidenceStage {
        self.stage
    }

    /// What a reader has to be able to open.
    #[must_use]
    pub fn admits(&self) -> &str {
        &self.admits
    }
}

/// The serialized shape of a [`RubricRow`].
#[derive(Debug, Serialize, Deserialize)]
struct RubricRowWire {
    criterion: CriterionId,
    stage: EvidenceStage,
    admits: String,
}

impl TryFrom<RubricRowWire> for RubricRow {
    type Error = CompetencyError;

    fn try_from(wire: RubricRowWire) -> Result<Self, Self::Error> {
        Self::of(wire.criterion, wire.stage, wire.admits)
    }
}

impl From<RubricRow> for RubricRowWire {
    fn from(value: RubricRow) -> Self {
        Self {
            criterion: value.criterion,
            stage: value.stage,
            admits: value.admits,
        }
    }
}

/// Every row of one competency's rubric.
///
/// No `Default`: a rubric with no rows witnesses nothing, and
/// [`crate::declare`] refuses one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceRubric {
    rows: Vec<RubricRow>,
}

impl EvidenceRubric {
    /// Takes the rows as they were written.
    #[must_use]
    pub const fn of(rows: Vec<RubricRow>) -> Self {
        Self { rows }
    }

    /// Every row, in the order they were written.
    #[must_use]
    pub fn rows(&self) -> &[RubricRow] {
        &self.rows
    }

    /// The row for one criterion at one stage, when there is one.
    ///
    /// A cell exists because the competency's author said this stage witnesses
    /// this criterion. There is no arm that invents a cell for a stage the
    /// rubric never mentioned.
    #[must_use]
    pub fn row(&self, criterion: &CriterionId, stage: EvidenceStage) -> Option<&RubricRow> {
        self.rows
            .iter()
            .find(|row| row.criterion() == criterion && row.stage() == stage)
    }

    /// Whether any row settles `criterion`.
    #[must_use]
    pub fn witnesses(&self, criterion: &CriterionId) -> bool {
        self.rows.iter().any(|row| row.criterion() == criterion)
    }
}
