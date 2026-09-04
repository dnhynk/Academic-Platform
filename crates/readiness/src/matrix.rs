//! Section 24.3's matrix: one row per competency of a bundle, six columns.
//!
//! ## The row has six columns and no seventh position to put a number in
//!
//! [`ReadinessRow`] has one field per column and nothing else that a reader
//! sees. The five evidence columns are [`AxisCell`]s and the sixth is a
//! [`FreshnessCell`], which is a different type, so the six are separate in the
//! sense the type checker can state: there is no position in a row where a band
//! could stand in for evidence, none where evidence could stand in for a band,
//! and none at all where a scalar over the whole row could stand.
//!
//! [`ReadinessRow::cells`] pairs [`crate::ReadinessAxis::ALL`] with those six
//! fields in order, and `six_axes_are_separate_columns` requires that pairing
//! to be a bijection — every axis reached once, no axis reached twice, and the
//! order the design document's own two places write.
//!
//! ## The matrix is taken of a bundle at an exact version
//!
//! [`take`] reads its row set out of a `P2-Y2` [`RoleProfile`]'s own
//! `competencies`, in that bundle's own order, and records the bundle by its
//! `RoleProfileRef` — the lineage-and-version pair. A matrix is therefore of one
//! version of one lineage and says so; `P2-R4` measured what a folded identity
//! costs, and a matrix keyed on a rendered name would be that shape one stage
//! up.
//!
//! A bundle entry whose competency the caller did not supply is **not** dropped
//! and not filled with a zero. It becomes a row whose every evidence column is
//! [`AxisCell::Missing`] and whose freshness column is
//! `FreshnessBand::Unknown`, which is section 24.3's own last table row —
//! `Distributed failure reasoning | exposure 없음 | — | — | — | — | Unknown` —
//! written as a value rather than as a rendering convention.

use academic_competency::{Competency, CompetencyId};
use academic_domain::FreshnessBand;
use academic_role_profile::{BundleImportance, RoleProfile, RoleProfileRef};
use serde::Serialize;

use crate::{
    axis::ReadinessAxis,
    cell::{AxisCell, AxisEvidence, FreshnessCell},
};

/// One competency's readiness, as one row of six columns.
///
/// Private fields, no setter, no `&mut self` method and no `Default`. A row is
/// produced by [`take`] and by nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadinessRow {
    competency: CompetencyId,
    importance: BundleImportance,
    academic_learning: AxisCell,
    problem_and_assignment: AxisCell,
    project_application: AxisCell,
    incident_debugging: AxisCell,
    design_choice: AxisCell,
    freshness: FreshnessCell,
}

impl ReadinessRow {
    /// Which competency the row is about.
    #[must_use]
    pub const fn competency(&self) -> &CompetencyId {
        &self.competency
    }

    /// What the bundle said this competency is worth to the role.
    ///
    /// The bundle's own [`BundleImportance`], carried and not scored. It is not
    /// a weight and no arithmetic here reads it: `P2-Y2` fixed it as one of
    /// section 24.2's three words.
    #[must_use]
    pub const fn importance(&self) -> BundleImportance {
        self.importance
    }

    /// The five evidence columns, in [`ReadinessAxis::ALL`] order.
    ///
    /// Total with no wildcard arm, so a seventh axis is a compile error here
    /// rather than a column that silently never renders.
    #[must_use]
    pub const fn evidence_cell(&self, axis: ReadinessAxis) -> Option<&AxisCell> {
        match axis {
            ReadinessAxis::AcademicLearning => Some(&self.academic_learning),
            ReadinessAxis::ProblemAndAssignment => Some(&self.problem_and_assignment),
            ReadinessAxis::ProjectApplication => Some(&self.project_application),
            ReadinessAxis::IncidentDebugging => Some(&self.incident_debugging),
            ReadinessAxis::DesignChoice => Some(&self.design_choice),
            ReadinessAxis::Freshness => None,
        }
    }

    /// The sixth column.
    #[must_use]
    pub const fn freshness(&self) -> FreshnessCell {
        self.freshness
    }

    /// Every column of this row, paired with the axis it renders under.
    ///
    /// One entry per [`ReadinessAxis::ALL`], in that order, and each entry
    /// carries either an evidence cell or a freshness cell — never both and
    /// never neither.
    #[must_use]
    pub fn cells(&self) -> Vec<(ReadinessAxis, ColumnReading<'_>)> {
        ReadinessAxis::ALL
            .into_iter()
            .map(|axis| {
                let reading = match self.evidence_cell(axis) {
                    Some(cell) => ColumnReading::Evidence(cell),
                    None => ColumnReading::Freshness(self.freshness),
                };
                (axis, reading)
            })
            .collect()
    }
}

/// What one column of one row holds.
///
/// Two arms and no third, because [`ReadinessAxis::is_freshness`] is total. The
/// enumeration exists so that a caller iterating a row has to say which of the
/// two it is looking at rather than reading a band as though it were evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "column", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ColumnReading<'row> {
    /// One of the five evidence columns.
    Evidence(&'row AxisCell),
    /// The freshness column.
    Freshness(FreshnessCell),
}

/// Section 24.3's matrix, of one bundle at one version.
///
/// Private fields, no setter and no `&mut self` method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadinessMatrix {
    bundle: RoleProfileRef,
    rows: Vec<ReadinessRow>,
}

impl ReadinessMatrix {
    /// Which bundle, at which version.
    #[must_use]
    pub const fn bundle(&self) -> &RoleProfileRef {
        &self.bundle
    }

    /// Every row, in the bundle's own entry order.
    #[must_use]
    pub fn rows(&self) -> &[ReadinessRow] {
        &self.rows
    }

    /// One row.
    #[must_use]
    pub fn row(&self, competency: &CompetencyId) -> Option<&ReadinessRow> {
        self.rows.iter().find(|row| row.competency() == competency)
    }
}

/// What a caller supplies for one competency of the bundle.
///
/// Three parts, all three required by the constructor. There is no `Default`
/// and no setter: a competency the caller knows nothing about is expressed by
/// leaving it out of the slice, which produces section 24.3's own all-missing
/// row rather than a row of invented zeroes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompetencyInput<'a> {
    competency: &'a Competency,
    placements: &'a [AxisEvidence],
    freshness: FreshnessBand,
}

impl<'a> CompetencyInput<'a> {
    /// Records what is known about one competency.
    #[must_use]
    pub const fn of(
        competency: &'a Competency,
        placements: &'a [AxisEvidence],
        freshness: FreshnessBand,
    ) -> Self {
        Self {
            competency,
            placements,
            freshness,
        }
    }

    /// The competency.
    #[must_use]
    pub const fn competency(&self) -> &'a Competency {
        self.competency
    }
}

/// Takes the matrix of one bundle.
///
/// A pure function of its two arguments. It reads no clock, opens nothing and
/// takes no `&mut`: a correction is a new call over new inputs, the way
/// `P2-Y2`'s revise and `P2-N2`'s supersede are.
///
/// The row set is the bundle's `competencies` in the bundle's own order. An
/// input naming a competency the bundle does not list reaches no row, because
/// the matrix is of the bundle rather than of whatever the caller happened to
/// pass.
#[must_use]
pub fn take(bundle: &RoleProfile, inputs: &[CompetencyInput<'_>]) -> ReadinessMatrix {
    let rows = bundle
        .competencies()
        .iter()
        .map(|entry| {
            let supplied = inputs
                .iter()
                .find(|input| input.competency.id() == entry.competency());
            let (
                academic_learning,
                problem_and_assignment,
                project_application,
                incident_debugging,
                design_choice,
                band,
            ) = match supplied {
                Some(input) => (
                    AxisCell::read(
                        ReadinessAxis::AcademicLearning,
                        input.competency,
                        input.placements,
                    ),
                    AxisCell::read(
                        ReadinessAxis::ProblemAndAssignment,
                        input.competency,
                        input.placements,
                    ),
                    AxisCell::read(
                        ReadinessAxis::ProjectApplication,
                        input.competency,
                        input.placements,
                    ),
                    AxisCell::read(
                        ReadinessAxis::IncidentDebugging,
                        input.competency,
                        input.placements,
                    ),
                    AxisCell::read(
                        ReadinessAxis::DesignChoice,
                        input.competency,
                        input.placements,
                    ),
                    input.freshness,
                ),
                None => (
                    AxisCell::Missing,
                    AxisCell::Missing,
                    AxisCell::Missing,
                    AxisCell::Missing,
                    AxisCell::Missing,
                    FreshnessBand::Unknown,
                ),
            };
            ReadinessRow {
                competency: entry.competency().clone(),
                importance: entry.importance(),
                academic_learning,
                problem_and_assignment,
                project_application,
                incident_debugging,
                design_choice,
                freshness: FreshnessCell::of(band),
            }
        })
        .collect();

    ReadinessMatrix {
        bundle: bundle.reference(),
        rows,
    }
}
