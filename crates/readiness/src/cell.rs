//! What one cell of the readiness matrix says, and the three readings that are
//! not one another.
//!
//! ## Missing, unknown and freshness are three types
//!
//! Section 34.5's `career readiness 과도한 점수화` row ends at
//! `missing/unknown과 freshness를 별도 표시`, and that separation is held by the
//! type system rather than by a rendering rule:
//!
//! | reading | what it means | where it lives |
//! |---|---|---|
//! | missing | nothing was recorded at this column | [`AxisCell::Missing`] |
//! | unknown | something was recorded and it settles nothing | [`AxisCell::Unknown`] |
//! | freshness | how recently the competency was exercised at all | [`FreshnessCell`], a different type |
//!
//! There is no conversion between [`AxisCell`] and [`FreshnessCell`] in either
//! direction, neither is a field of the other, and
//! `missing_and_unknown_are_separate_from_freshness` compares the whole set of
//! this crate's public signatures for one. `academic_domain::FreshnessBand` has
//! its own `Unknown`, spelled the same as [`AxisCell::Unknown`] and meaning
//! something else — a band for a concept about which nothing datable was ever
//! admitted, which `P2-N3` fixed and this crate does not restate. The shared
//! spelling is exactly why the two are two types.
//!
//! ## The axis is recorded, and no function derives it
//!
//! `P2-Y1` recorded that section 13.2's eight evidence rows and section 24.3's
//! six evidence stages are a coincidence of counts and not a correspondence,
//! because a total map between them would have to invent three of its six
//! answers. The same is true one layer up and more sharply: section 24.3's
//! `설계 선택` column has no section 13.2 row at all, so a function from
//! [`academic_knowledge_state::EvidenceKind`] or
//! [`academic_competency::EvidenceStage`] to [`crate::ReadinessAxis`] would have
//! to invent that column's answer.
//!
//! So this crate builds none. [`AxisEvidence::place`] takes the axis **as an
//! argument**: the column is where the user put the evidence, and the record's
//! own stage and origin travel beside it so a reader sees both.
//! `no_function_maps_a_stage_or_a_kind_to_an_axis` compares the whole set of
//! public signatures in this crate against that absence.
//!
//! ## A cell is read, not asserted
//!
//! [`AxisCell::read`] is a pure function of one axis, one competency and the
//! placements recorded against it. There is no constructor that takes a reading
//! directly, so `Evidenced` is not a value a caller can write beside no
//! evidence, and `Missing` is not a value a caller can write over evidence that
//! exists.

use academic_competency::{Competency, CriterionId, EvidenceStage, StageEvidence};
use academic_domain::FreshnessBand;
use serde::Serialize;

use crate::{ReadinessError, axis::ReadinessAxis, identity::EvidenceLocatorId};

/// Section 24.3's own spelling for a cell nothing was recorded in.
pub const MISSING_CELL_MARK: &str = "—";

/// One piece of evidence, placed by the user in one column.
///
/// The placement is the user's; the record, the stage and the criterion are
/// not. [`AxisEvidence::place`] takes `P2-Y1`'s own [`StageEvidence`] and reads
/// the stage out of it, so a placement cannot claim a depth the record does not
/// carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AxisEvidence {
    axis: ReadinessAxis,
    criterion: CriterionId,
    stage: EvidenceStage,
    locator: EvidenceLocatorId,
    record: StageEvidence,
}

impl AxisEvidence {
    /// Places one `P2-Y1` record in one column, against one criterion.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::FreshnessIsNotAnEvidenceColumn`] when the column is
    /// [`ReadinessAxis::Freshness`], which carries a band and never a locator.
    pub fn place(
        axis: ReadinessAxis,
        criterion: CriterionId,
        locator: EvidenceLocatorId,
        record: &StageEvidence,
    ) -> Result<Self, ReadinessError> {
        if axis.is_freshness() {
            return Err(ReadinessError::FreshnessIsNotAnEvidenceColumn);
        }
        Ok(Self {
            axis,
            criterion,
            stage: record.stage(),
            locator,
            record: record.clone(),
        })
    }

    /// Which column the user placed it in.
    #[must_use]
    pub const fn axis(&self) -> ReadinessAxis {
        self.axis
    }

    /// Which performance criterion it is offered as evidence for.
    #[must_use]
    pub const fn criterion(&self) -> &CriterionId {
        &self.criterion
    }

    /// The record's own section 24.3 stage, read out of the record.
    #[must_use]
    pub const fn stage(&self) -> EvidenceStage {
        self.stage
    }

    /// Where a reader opens it.
    #[must_use]
    pub const fn locator(&self) -> &EvidenceLocatorId {
        &self.locator
    }

    /// The `P2-Y1` record itself.
    #[must_use]
    pub const fn record(&self) -> &StageEvidence {
        &self.record
    }
}

/// Why a column that carries something still says nothing.
///
/// Both arms are `P2-Y1`'s own readings one layer up: the first is a placement
/// its sheet would leave in `unmatched`, and the second is its
/// `CellState::NotInRubric`. Neither is *missing* data — something was recorded
/// in both — and neither is a freshness band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnknownBasis {
    /// The placement names a criterion this competency does not state.
    NamesNoStatedCriterion,
    /// The competency's rubric admits no row at the placement's own stage for
    /// the criterion it names, so nothing was ever going to witness it there.
    RubricAdmitsNoRowAtThatStage,
}

impl UnknownBasis {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [
        Self::NamesNoStatedCriterion,
        Self::RubricAdmitsNoRowAtThatStage,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NamesNoStatedCriterion => "NAMES_NO_STATED_CRITERION",
            Self::RubricAdmitsNoRowAtThatStage => "RUBRIC_ADMITS_NO_ROW_AT_THAT_STAGE",
        }
    }
}

/// One placement the column refused, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefusedPlacement {
    basis: UnknownBasis,
    evidence: AxisEvidence,
}

impl RefusedPlacement {
    /// Why the column could not read it.
    #[must_use]
    pub const fn basis(&self) -> UnknownBasis {
        self.basis
    }

    /// The placement itself, so a reader can still open what arrived.
    #[must_use]
    pub const fn evidence(&self) -> &AxisEvidence {
        &self.evidence
    }
}

/// What one of the five evidence columns says.
///
/// Three readings, no constructor that takes one directly, and no fourth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "reading",
    content = "detail",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum AxisCell {
    /// At least one placement settles this column. Never empty.
    ///
    /// `#[non_exhaustive]`, so another crate cannot write one: an *empty*
    /// filled cell would say a column is settled by nothing, and
    /// [`AxisCell::read`] is the one producer.
    #[non_exhaustive]
    Evidenced(Vec<AxisEvidence>),
    /// Nothing was recorded in this column at all. Section 24.3's `—`.
    Missing,
    /// Something was recorded here and none of it settles the column.
    ///
    /// `#[non_exhaustive]` for the same reason: an empty refusal list would say
    /// a column is unreadable because of nothing.
    #[non_exhaustive]
    Unknown(Vec<RefusedPlacement>),
}

impl AxisCell {
    /// Reads one column of one competency's row from the placements recorded
    /// against it.
    ///
    /// A pure function of its three arguments. Placements at other columns are
    /// ignored here and are read by the call for their own column, so a
    /// placement reaches exactly one cell.
    #[must_use]
    pub fn read(axis: ReadinessAxis, competency: &Competency, placed: &[AxisEvidence]) -> Self {
        if axis.is_freshness() {
            return Self::Missing;
        }
        let mut settling = Vec::new();
        let mut refused = Vec::new();
        for evidence in placed.iter().filter(|item| item.axis() == axis) {
            if competency.criterion(evidence.criterion()).is_none() {
                refused.push(RefusedPlacement {
                    basis: UnknownBasis::NamesNoStatedCriterion,
                    evidence: evidence.clone(),
                });
            } else if competency
                .rubric()
                .row(evidence.criterion(), evidence.stage())
                .is_some()
            {
                settling.push(evidence.clone());
            } else {
                refused.push(RefusedPlacement {
                    basis: UnknownBasis::RubricAdmitsNoRowAtThatStage,
                    evidence: evidence.clone(),
                });
            }
        }
        if settling.is_empty() {
            if refused.is_empty() {
                Self::Missing
            } else {
                Self::Unknown(refused)
            }
        } else {
            Self::Evidenced(settling)
        }
    }

    /// Stable spelling of the reading.
    ///
    /// Total, with no wildcard arm.
    #[must_use]
    pub const fn reading(&self) -> &'static str {
        match self {
            Self::Evidenced(_) => "EVIDENCED",
            Self::Missing => "MISSING",
            Self::Unknown(_) => "UNKNOWN",
        }
    }

    /// Every reading's spelling, in this enumeration's own order.
    pub const READINGS: [&'static str; 3] = ["EVIDENCED", "MISSING", "UNKNOWN"];

    /// The placements that settle it, which is empty unless it is evidenced.
    #[must_use]
    pub fn settled_by(&self) -> &[AxisEvidence] {
        match self {
            Self::Evidenced(items) => items,
            Self::Missing | Self::Unknown(_) => &[],
        }
    }

    /// The placements it refused, which is empty unless it is unknown.
    #[must_use]
    pub fn refused(&self) -> &[RefusedPlacement] {
        match self {
            Self::Unknown(items) => items,
            Self::Evidenced(_) | Self::Missing => &[],
        }
    }
}

/// What the sixth column says.
///
/// A different type from [`AxisCell`], carrying `P2-N3`'s band and nothing
/// else. There is no `From` in either direction and neither is a field of the
/// other, which is what `missing/unknown과 freshness를 별도 표시` means when it
/// is a property of the program rather than of a stylesheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FreshnessCell {
    band: FreshnessBand,
}

impl FreshnessCell {
    /// Records the band `P2-N3` projected for this competency.
    #[must_use]
    pub const fn of(band: FreshnessBand) -> Self {
        Self { band }
    }

    /// The band.
    #[must_use]
    pub const fn band(&self) -> FreshnessBand {
        self.band
    }
}
