//! Section 24.3's auxiliary score, and the four disclosures without which it
//! does not exist.
//!
//! Section 24.3's last sentence is the whole contract:
//!
//! > `보조 score가 필요하면 각 cell의 rubric, source, 누락 데이터와 가중치를
//! > 공개하고 비교·채용 가능성을 보장하는 수치가 아님을 표시한다.`
//!
//! Four things are published — the rubric, the source, the missing data and the
//! weights — and the number is marked as not a guarantee. The four are not a
//! checklist run before a number is built; they are the only way the number
//! comes into existence.
//!
//! ## The number is not an argument
//!
//! [`disclose`] is the **one** producer of an [`AuxiliaryScore`], and there is
//! no score parameter in it. The value is computed from the disclosed weights
//! over the disclosed matrix, so there is no expression anywhere that produces a
//! score somebody chose. `P2-N6`'s *five disclosure groups without which no
//! result exists*, `P2-L5`'s `AccuracyWitness` taken by value, and `P2-Y1`'s
//! *the statement is rendered and never supplied* are the same shape; this is
//! the one where the thing that cannot be supplied is a number.
//!
//! ## Three of the four are re-derived and refused if they disagree
//!
//! A disclosure that could be written freehand would be a disclosure of
//! whatever the author preferred. [`RubricDisclosure`], [`SourceDisclosure`] and
//! [`MissingDataDisclosure`] each have one producer, each takes the matrix (and,
//! for the rubric, the competencies the matrix is of), and [`disclose`]
//! **re-derives all three and refuses any that is not equal to its own
//! derivation**. So a disclosure taken of a different matrix, or of the same
//! matrix before a cell changed, is refused rather than published beside a
//! number it does not describe.
//!
//! The fourth, [`WeightDisclosure`], is the user's own judgement and is not
//! derivable from anything. What is checked is that it is *total*: a weight for
//! every evidence column of [`crate::ReadinessAxis`], each with the user's own
//! stated reason, so a column silently left out of the weighting is refused
//! rather than defaulted to zero.
//!
//! ## It carries no float and performs no division
//!
//! [`ScoreValue`] is two `u32` counts of weighted units. Nothing in this crate
//! declares an `f32` or an `f64` or divides, so the *허위 정밀도* section 34.5
//! names has no type to arrive in. `no_primary_aggregate_percentage` compares
//! the whole set of declared field types and the whole set of public return
//! types against that absence, in both directions.

use std::collections::BTreeSet;

use academic_competency::{Competency, CompetencyId};
use serde::Serialize;

use crate::{
    ReadinessError,
    axis::ReadinessAxis,
    cell::AxisCell,
    identity::{EvidenceLocatorId, non_empty},
    matrix::ReadinessMatrix,
};

/// One competency's rubric, as the reader of a score sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RubricLines {
    competency: CompetencyId,
    lines: Vec<String>,
}

impl RubricLines {
    /// Which competency.
    #[must_use]
    pub const fn competency(&self) -> &CompetencyId {
        &self.competency
    }

    /// The rubric rows' own `admits` text, in the rubric's own order.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

/// Section 24.3's `각 cell의 rubric`.
///
/// One producer, [`RubricDisclosure::of`], which reads the rubric out of
/// `P2-Y1`'s own [`Competency`] values. There is no constructor that takes a
/// line, so the published rubric is the one the competency was declared with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RubricDisclosure {
    entries: Vec<RubricLines>,
}

impl RubricDisclosure {
    /// Reads the rubric of every row of `matrix` out of `competencies`.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::DisclosureDoesNotCoverTheMatrix`] when a row of the
    /// matrix has no competency in `competencies`.
    pub fn of(
        matrix: &ReadinessMatrix,
        competencies: &[&Competency],
    ) -> Result<Self, ReadinessError> {
        let mut entries = Vec::new();
        for row in matrix.rows() {
            let competency = competencies
                .iter()
                .find(|item| item.id() == row.competency())
                .ok_or_else(|| {
                    ReadinessError::DisclosureDoesNotCoverTheMatrix(
                        "rubric",
                        row.competency().as_str().to_owned(),
                    )
                })?;
            entries.push(RubricLines {
                competency: row.competency().clone(),
                lines: competency
                    .rubric()
                    .rows()
                    .iter()
                    .map(|item| format!("{}: {}", item.stage().as_str(), item.admits()))
                    .collect(),
            });
        }
        Ok(Self { entries })
    }

    /// One entry per row of the matrix, in the matrix's own order.
    #[must_use]
    pub fn entries(&self) -> &[RubricLines] {
        &self.entries
    }
}

/// Section 24.3's `source`.
///
/// One producer, [`SourceDisclosure::of`], which collects the locator of every
/// placement that settles a cell of the matrix. A score's sources are therefore
/// exactly what a reader can open, and there is no constructor that takes a
/// citation somebody typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceDisclosure {
    locators: Vec<EvidenceLocatorId>,
}

impl SourceDisclosure {
    /// Collects every locator the matrix's evidenced cells rest on, sorted and
    /// deduplicated.
    #[must_use]
    pub fn of(matrix: &ReadinessMatrix) -> Self {
        let mut seen: BTreeSet<EvidenceLocatorId> = BTreeSet::new();
        for row in matrix.rows() {
            for axis in ReadinessAxis::ALL {
                if let Some(cell) = row.evidence_cell(axis) {
                    for evidence in cell.settled_by() {
                        seen.insert(evidence.locator().clone());
                    }
                }
            }
        }
        Self {
            locators: seen.into_iter().collect(),
        }
    }

    /// Every locator, sorted.
    #[must_use]
    pub fn locators(&self) -> &[EvidenceLocatorId] {
        &self.locators
    }
}

/// One cell the score had no evidence for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingDatum {
    competency: CompetencyId,
    axis: ReadinessAxis,
    reading: &'static str,
}

impl MissingDatum {
    /// Which competency.
    #[must_use]
    pub const fn competency(&self) -> &CompetencyId {
        &self.competency
    }

    /// Which column.
    #[must_use]
    pub const fn axis(&self) -> ReadinessAxis {
        self.axis
    }

    /// The cell's own reading, which is `MISSING` or `UNKNOWN` and never
    /// `EVIDENCED`.
    #[must_use]
    pub const fn reading(&self) -> &'static str {
        self.reading
    }
}

/// Section 24.3's `누락 데이터`.
///
/// One producer, [`MissingDataDisclosure::of`], which walks every cell of the
/// matrix. A score cannot be published claiming complete data over a matrix
/// full of holes, because the claim is not an input.
///
/// Missing and unknown are listed **separately**, by the cell's own reading, and
/// the freshness column is not listed at all: a band is not missing data, which
/// is section 34.5's `missing/unknown과 freshness를 별도 표시` in the one place
/// where folding them would have been convenient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingDataDisclosure {
    entries: Vec<MissingDatum>,
}

impl MissingDataDisclosure {
    /// Walks every evidence cell of the matrix.
    #[must_use]
    pub fn of(matrix: &ReadinessMatrix) -> Self {
        let mut entries = Vec::new();
        for row in matrix.rows() {
            for axis in ReadinessAxis::ALL {
                let Some(cell) = row.evidence_cell(axis) else {
                    continue;
                };
                match cell {
                    AxisCell::Evidenced(_) => {}
                    AxisCell::Missing | AxisCell::Unknown(_) => entries.push(MissingDatum {
                        competency: row.competency().clone(),
                        axis,
                        reading: cell.reading(),
                    }),
                }
            }
        }
        Self { entries }
    }

    /// Every cell the score had nothing for.
    #[must_use]
    pub fn entries(&self) -> &[MissingDatum] {
        &self.entries
    }

    /// How many of them read `MISSING`.
    #[must_use]
    pub fn missing_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.reading == AxisCell::Missing.reading())
            .count()
    }

    /// How many of them read `UNKNOWN`.
    #[must_use]
    pub fn unknown_count(&self) -> usize {
        self.entries.len() - self.missing_count()
    }
}

/// One column's weight, with the user's own reason for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AxisWeight {
    axis: ReadinessAxis,
    weight: u32,
    reason: String,
}

impl AxisWeight {
    /// Records one column's weight and why the user chose it.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::FreshnessIsNotAnEvidenceColumn`] when the column is
    /// the freshness one, which is not weighted because it is not evidence; and
    /// [`ReadinessError::EmptyText`] when no reason was written, because a
    /// weight nobody explained is a weight that was not disclosed.
    pub fn of(
        axis: ReadinessAxis,
        weight: u32,
        reason: impl Into<String>,
    ) -> Result<Self, ReadinessError> {
        if axis.is_freshness() {
            return Err(ReadinessError::FreshnessIsNotAnEvidenceColumn);
        }
        Ok(Self {
            axis,
            weight,
            reason: non_empty(reason.into(), "weight reason")?,
        })
    }

    /// Which column.
    #[must_use]
    pub const fn axis(&self) -> ReadinessAxis {
        self.axis
    }

    /// Its weight in units.
    #[must_use]
    pub const fn weight(&self) -> u32 {
        self.weight
    }

    /// Why the user chose it.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Section 24.3's `가중치`.
///
/// The one disclosure that is not derivable, and the one that is checked for
/// being *total*: [`WeightDisclosure::of`] requires exactly one entry for every
/// evidence column of [`ReadinessAxis`], so a column left out of the weighting
/// is refused rather than weighted at zero by omission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeightDisclosure {
    weights: Vec<AxisWeight>,
}

impl WeightDisclosure {
    /// Records the user's weighting of every evidence column.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::WeightingIsNotTotal`] when the entries are not exactly
    /// one per evidence column of [`ReadinessAxis`].
    pub fn of(weights: Vec<AxisWeight>) -> Result<Self, ReadinessError> {
        let declared: Vec<ReadinessAxis> = weights.iter().map(AxisWeight::axis).collect();
        let mut sorted = declared.clone();
        sorted.sort_unstable();
        sorted.dedup();
        let expected = ReadinessAxis::evidence_axes();
        if sorted != expected {
            return Err(ReadinessError::WeightingIsNotTotal);
        }
        Ok(Self { weights })
    }

    /// Every weight, in the order the user wrote them.
    #[must_use]
    pub fn weights(&self) -> &[AxisWeight] {
        &self.weights
    }

    /// One column's weight.
    #[must_use]
    pub fn weight_of(&self, axis: ReadinessAxis) -> Option<u32> {
        self.weights
            .iter()
            .find(|entry| entry.axis() == axis)
            .map(AxisWeight::weight)
    }
}

/// The number, in weighted units.
///
/// Two counts and no ratio: nothing here divides and nothing here is a float,
/// so the number a reader sees is `12 / 30 weighted units` and not a percentage
/// that would claim a precision the evidence does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ScoreValue {
    evidenced_units: u32,
    weighted_units: u32,
}

impl ScoreValue {
    /// The weight of every evidenced cell.
    #[must_use]
    pub const fn evidenced_units(self) -> u32 {
        self.evidenced_units
    }

    /// The weight of every cell, evidenced or not.
    #[must_use]
    pub const fn weighted_units(self) -> u32 {
        self.weighted_units
    }
}

/// Section 24.3's auxiliary score, with its four disclosures attached.
///
/// No public field, no setter, no `Default` and no `Deserialize`: it is
/// produced by [`disclose`] and by nothing else, and a document could not carry
/// one back in without its disclosures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuxiliaryScore {
    value: ScoreValue,
    rubric: RubricDisclosure,
    sources: SourceDisclosure,
    missing_data: MissingDataDisclosure,
    weights: WeightDisclosure,
}

impl AuxiliaryScore {
    /// The number.
    #[must_use]
    pub const fn value(&self) -> ScoreValue {
        self.value
    }

    /// The published rubric.
    #[must_use]
    pub const fn rubric(&self) -> &RubricDisclosure {
        &self.rubric
    }

    /// The published sources.
    #[must_use]
    pub const fn sources(&self) -> &SourceDisclosure {
        &self.sources
    }

    /// The published missing data.
    #[must_use]
    pub const fn missing_data(&self) -> &MissingDataDisclosure {
        &self.missing_data
    }

    /// The published weights.
    #[must_use]
    pub const fn weights(&self) -> &WeightDisclosure {
        &self.weights
    }
}

/// Publishes one auxiliary score over `matrix`, with its four disclosures.
///
/// The one producer of an [`AuxiliaryScore`]. There is no score parameter: the
/// value is computed from `weights` over `matrix`.
///
/// # Errors
///
/// [`ReadinessError::DisclosureDoesNotCoverTheMatrix`] when the rubric, the
/// sources or the missing-data disclosure is not the one this matrix and these
/// competencies produce — which is what a disclosure taken of a different
/// matrix looks like from here; and
/// [`ReadinessError::ScoreWouldOverflow`] when the weighting is large enough
/// that the total does not fit, because a wrapped total would be a smaller
/// number claiming to be the whole.
pub fn disclose(
    matrix: &ReadinessMatrix,
    competencies: &[&Competency],
    rubric: RubricDisclosure,
    sources: SourceDisclosure,
    missing_data: MissingDataDisclosure,
    weights: WeightDisclosure,
) -> Result<AuxiliaryScore, ReadinessError> {
    if rubric != RubricDisclosure::of(matrix, competencies)? {
        return Err(ReadinessError::DisclosureDoesNotCoverTheMatrix(
            "rubric",
            matrix.bundle().rendered(),
        ));
    }
    if sources != SourceDisclosure::of(matrix) {
        return Err(ReadinessError::DisclosureDoesNotCoverTheMatrix(
            "source",
            matrix.bundle().rendered(),
        ));
    }
    if missing_data != MissingDataDisclosure::of(matrix) {
        return Err(ReadinessError::DisclosureDoesNotCoverTheMatrix(
            "missing data",
            matrix.bundle().rendered(),
        ));
    }

    let mut evidenced_units: u32 = 0;
    let mut weighted_units: u32 = 0;
    for row in matrix.rows() {
        for axis in ReadinessAxis::ALL {
            let Some(cell) = row.evidence_cell(axis) else {
                continue;
            };
            let weight = weights
                .weight_of(axis)
                .ok_or(ReadinessError::WeightingIsNotTotal)?;
            weighted_units = weighted_units
                .checked_add(weight)
                .ok_or(ReadinessError::ScoreWouldOverflow)?;
            if matches!(cell, AxisCell::Evidenced(_)) {
                evidenced_units = evidenced_units
                    .checked_add(weight)
                    .ok_or(ReadinessError::ScoreWouldOverflow)?;
            }
        }
    }

    Ok(AuxiliaryScore {
        value: ScoreValue {
            evidenced_units,
            weighted_units,
        },
        rubric,
        sources,
        missing_data,
        weights,
    })
}
