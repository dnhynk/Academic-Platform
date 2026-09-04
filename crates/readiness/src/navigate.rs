//! Section 24.4's four directions, and the two things a walk may end at.
//!
//! Section 24.4 lists four directions and then states the constraint on all
//! four in one sentence:
//!
//! > `어느 방향에서도 "직무에 중요"라는 추상 문구로 끝나지 않고, 수행 criterion과
//! > 실제 개인 evidence까지 drill down할 수 있다.`
//!
//! **The count is not asserted as a number here.**
//! `four_navigation_directions_terminate_at_criterion_and_evidence` parses
//! section 24.4's own `- ` bullets out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares them
//! against [`NavigationDirection::ALL`] in both directions, so four is a
//! measurement of the design document.
//!
//! ## The direction is not a second argument
//!
//! [`traverse`] takes one [`StartingPoint`], whose four arms are the four
//! directions, and reads the direction off it. So a walk cannot be asked to go
//! `FromCourse` from a project: the pair that could disagree does not exist.
//!
//! Each arm also carries the *typed* identity its direction is about — a
//! `P2-Y1` [`ConceptRef`], which is a namespace and a value rather than a bare
//! string; a `P2-Y2` [`RoleProfileRef`], which is a lineage and a version rather
//! than section 24.2's folded spelling; and this crate's
//! [`StartingPointId`] for the two that name an outside record. Nothing here
//! compares an ontology identifier against a classification token, which is the
//! fold `ConceptRef` exists to prevent and `P2-R4` measured the cost of.
//!
//! ## No walk can end nowhere
//!
//! [`traverse`] returns a [`Termination`], whose one constructor takes its
//! first [`Terminus`] **by value**. An empty result is therefore not a value
//! that exists, and there is no arm of [`traverse`] that returns without
//! producing at least one terminus: a direction whose starting point reaches no
//! row of the matrix ends at
//! [`AbsenceState::NoRowReachesTheStartingPoint`], which names the direction and
//! the starting point.
//!
//! ## No walk can end in prose
//!
//! [`Terminus`] has two arms and neither carries a sentence. Every field of
//! both is a typed identifier, an axis or a closed enumeration issued by
//! `P2-Y1`, `P2-Y2` or this crate, so `직무에 중요` has no field to arrive in.
//! `no_terminus_carries_free_prose` compares the whole set of the declared
//! fields of both arms against that, in both directions.
//!
//! ## The matrix a walk runs over is never empty
//!
//! `P2-Y2`'s `declare` refuses a bundle that names no competency and `P2-Y1`'s
//! refuses a competency that states no criterion, so a matrix taken of a bundle
//! has at least one row and every row has at least one criterion.
//! `a_walk_over_a_matrix_cannot_run_zero_times` measures both refusals rather
//! than assuming them, and this crate adds no third check of its own: a branch
//! guarding a case its own inputs cannot produce is a branch that never runs,
//! which is the defect `P2-R5` measured.
//!
//! What does the work here instead is the fallback in [`traverse`]: a walk that
//! reached no row at all still produces
//! [`AbsenceState::NoRowReachesTheStartingPoint`], and that arm *is* reached —
//! `four_navigation_directions_terminate_at_criterion_and_evidence` walks four
//! starting points that match nothing and requires it.

use academic_competency::{CompetencyId, ConceptRef, CriterionId, EvidenceSource};
use academic_role_profile::RoleProfileRef;
use serde::Serialize;

use crate::{
    axis::ReadinessAxis,
    cell::{AxisCell, AxisEvidence, UnknownBasis},
    identity::{EvidenceLocatorId, StartingPointId},
    view::ReadinessView,
};

/// One of section 24.4's four directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NavigationDirection {
    /// `Concept → 사용되는 system/competency/role/project/course`
    FromConcept,
    /// `Goal/Role → 필요한 competency → enabling concept → prerequisite`
    FromGoalOrRole,
    /// `Project → observed/required/beneficial concept → 학교/외부 acquisition option`
    FromProject,
    /// `Course → designed/actual coverage → competency → project/role relevance`
    FromCourse,
}

impl NavigationDirection {
    /// Exhaustive, in section 24.4's own bullet order.
    pub const ALL: [Self; 4] = [
        Self::FromConcept,
        Self::FromGoalOrRole,
        Self::FromProject,
        Self::FromCourse,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FromConcept => "FROM_CONCEPT",
            Self::FromGoalOrRole => "FROM_GOAL_OR_ROLE",
            Self::FromProject => "FROM_PROJECT",
            Self::FromCourse => "FROM_COURSE",
        }
    }

    /// Section 24.4's own bullet for this direction, verbatim.
    #[must_use]
    pub const fn specification_bullet(self) -> &'static str {
        match self {
            Self::FromConcept => "Concept → 사용되는 system/competency/role/project/course",
            Self::FromGoalOrRole => {
                "Goal/Role → 필요한 competency → enabling concept → prerequisite"
            }
            Self::FromProject => {
                "Project → observed/required/beneficial concept → 학교/외부 acquisition option"
            }
            Self::FromCourse => {
                "Course → designed/actual coverage → competency → project/role relevance"
            }
        }
    }
}

/// Where one walk starts, in the identity its direction is about.
///
/// Four arms, one per direction, and the direction is read off the arm rather
/// than passed beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "start", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StartingPoint {
    /// `P2-Y1`'s concept reference, namespace included.
    Concept(ConceptRef),
    /// `P2-Y2`'s bundle, at an exact version.
    GoalOrRole(RoleProfileRef),
    /// `P2-R5`'s personal application claim, by its identifier.
    Project(StartingPointId),
    /// `P2-N2`'s admitted evidence, by its identifier.
    Course(StartingPointId),
}

impl StartingPoint {
    /// Which of section 24.4's directions this start walks.
    ///
    /// Total, with no wildcard arm.
    #[must_use]
    pub const fn direction(&self) -> NavigationDirection {
        match self {
            Self::Concept(_) => NavigationDirection::FromConcept,
            Self::GoalOrRole(_) => NavigationDirection::FromGoalOrRole,
            Self::Project(_) => NavigationDirection::FromProject,
            Self::Course(_) => NavigationDirection::FromCourse,
        }
    }

    /// Whether one placement is one this start reaches.
    ///
    /// Total over both enumerations with no wildcard arm, so a third `P2-Y1`
    /// evidence origin or a fifth direction is a compile error here rather than
    /// a walk that silently matches nothing.
    fn reaches(&self, evidence: &AxisEvidence) -> bool {
        match self {
            // The goal *is* the bundle, so every row of its own matrix is
            // reached and no placement has to match.
            Self::GoalOrRole(_) => true,
            // Whole-pair, namespace included: an ontology identifier spelled
            // like a classification token is a different concept.
            Self::Concept(concept) => evidence.record().concept() == concept,
            Self::Project(claim) => match evidence.record().source() {
                EvidenceSource::PersonalApplication(id) => id == claim.as_str(),
                EvidenceSource::KnowledgeState(_) => false,
            },
            Self::Course(item) => match evidence.record().source() {
                EvidenceSource::KnowledgeState(id) => id.to_string() == item.as_str(),
                EvidenceSource::PersonalApplication(_) => false,
            },
        }
    }
}

/// An absence a walk ends at, named exactly.
///
/// Three arms and no free text in any of them. This is the *명시적 결측 상태*
/// half of the contract: a walk that found nothing says which competency, which
/// criterion and which column, or — when the starting point reached no row at
/// all — which direction and which starting point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "absence", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AbsenceState {
    /// Nothing was recorded in this column of this competency.
    CellIsMissing {
        /// Which competency.
        competency: CompetencyId,
        /// Which criterion the walk reached.
        criterion: CriterionId,
        /// Which column.
        axis: ReadinessAxis,
    },
    /// Something was recorded in this column and it settles nothing.
    CellIsUnknown {
        /// Which competency.
        competency: CompetencyId,
        /// Which criterion the walk reached.
        criterion: CriterionId,
        /// Which column.
        axis: ReadinessAxis,
        /// Why the column could not read what arrived.
        basis: UnknownBasis,
    },
    /// The starting point named no row of this matrix.
    NoRowReachesTheStartingPoint {
        /// Which direction was walked.
        direction: NavigationDirection,
        /// What it started from.
        start: StartingPoint,
    },
}

/// Where one path of a walk ends.
///
/// Two arms, and section 24.4's `추상 문구` is neither of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "terminus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Terminus {
    /// A performance criterion and a locator a reader opens.
    CriterionAndEvidence {
        /// Which competency.
        competency: CompetencyId,
        /// Section 24.1's performance criterion.
        criterion: CriterionId,
        /// Which column it is displayed in.
        axis: ReadinessAxis,
        /// Where a reader opens the evidence.
        locator: EvidenceLocatorId,
    },
    /// An explicit missing state.
    ExplicitAbsence(AbsenceState),
}

impl Terminus {
    /// Stable spelling of which of the two this is.
    ///
    /// Total, with no wildcard arm.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::CriterionAndEvidence { .. } => "CRITERION_AND_EVIDENCE",
            Self::ExplicitAbsence(_) => "EXPLICIT_ABSENCE",
        }
    }

    /// Every kind's spelling, in this enumeration's own order.
    pub const KINDS: [&'static str; 2] = ["CRITERION_AND_EVIDENCE", "EXPLICIT_ABSENCE"];

    /// The performance criterion this path reached, when it reached one.
    ///
    /// `None` only for [`AbsenceState::NoRowReachesTheStartingPoint`], which is
    /// the one terminus that is about the walk rather than about a competency.
    #[must_use]
    pub const fn criterion(&self) -> Option<&CriterionId> {
        match self {
            Self::CriterionAndEvidence { criterion, .. }
            | Self::ExplicitAbsence(
                AbsenceState::CellIsMissing { criterion, .. }
                | AbsenceState::CellIsUnknown { criterion, .. },
            ) => Some(criterion),
            Self::ExplicitAbsence(AbsenceState::NoRowReachesTheStartingPoint { .. }) => None,
        }
    }
}

/// Every path one walk ended at, and never none of them.
///
/// The first terminus is a field taken by value, so an empty termination is not
/// a value that exists. `P2-U3`'s `INDETERMINATE` verdict takes its first
/// missing check the same way, for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Termination {
    direction: NavigationDirection,
    start: StartingPoint,
    first: Terminus,
    rest: Vec<Terminus>,
}

impl Termination {
    /// Which direction was walked.
    #[must_use]
    pub const fn direction(&self) -> NavigationDirection {
        self.direction
    }

    /// What it started from.
    #[must_use]
    pub const fn start(&self) -> &StartingPoint {
        &self.start
    }

    /// Every terminus, first one included. Never empty.
    #[must_use]
    pub fn termini(&self) -> Vec<&Terminus> {
        let mut all = vec![&self.first];
        all.extend(self.rest.iter());
        all
    }
}

/// Walks `view`'s matrix from one starting point.
///
/// A pure function of its two arguments. The start reaches a set of rows and
/// the walk then visits every evidence column of every row it reached, ending
/// each path at a criterion with a locator or at an explicit absence.
///
/// | direction | the rows it reaches |
/// |---|---|
/// | `FromConcept` | every row holding a placement whose `P2-Y1` record is about that exact concept reference |
/// | `FromGoalOrRole` | every row of the bundle, because the goal *is* the bundle |
/// | `FromProject` | every row holding a placement founded on `P2-R5`'s claim named by the start |
/// | `FromCourse` | every row holding a placement founded on the `P2-N2` evidence named by the start |
#[must_use]
pub fn traverse(view: &ReadinessView, start: &StartingPoint) -> Termination {
    let direction = start.direction();
    let mut termini: Vec<Terminus> = Vec::new();

    for row in view.matrix().rows() {
        let reached = ReadinessAxis::ALL.into_iter().any(|axis| {
            row.evidence_cell(axis).is_some_and(|cell| {
                cell.settled_by()
                    .iter()
                    .chain(
                        cell.refused()
                            .iter()
                            .map(super::cell::RefusedPlacement::evidence),
                    )
                    .any(|evidence| start.reaches(evidence))
            })
        }) || matches!(start, StartingPoint::GoalOrRole(_));
        if !reached {
            continue;
        }

        for axis in ReadinessAxis::ALL {
            let Some(cell) = row.evidence_cell(axis) else {
                continue;
            };
            match cell {
                AxisCell::Evidenced(placements) => {
                    for placement in placements {
                        termini.push(Terminus::CriterionAndEvidence {
                            competency: row.competency().clone(),
                            criterion: placement.criterion().clone(),
                            axis,
                            locator: placement.locator().clone(),
                        });
                    }
                }
                AxisCell::Unknown(refused) => {
                    for item in refused {
                        termini.push(Terminus::ExplicitAbsence(AbsenceState::CellIsUnknown {
                            competency: row.competency().clone(),
                            criterion: item.evidence().criterion().clone(),
                            axis,
                            basis: item.basis(),
                        }));
                    }
                }
                AxisCell::Missing => {
                    // Section 24.4 ends a walk at a criterion, so an empty
                    // column ends at each criterion the competency states
                    // rather than at the column alone. `P2-Y1`'s `declare`
                    // refuses a competency with no criterion, so this inner
                    // loop cannot run zero times for a row of a view.
                    for criterion in view.criteria_of(row.competency()) {
                        termini.push(Terminus::ExplicitAbsence(AbsenceState::CellIsMissing {
                            competency: row.competency().clone(),
                            criterion: criterion.clone(),
                            axis,
                        }));
                    }
                }
            }
        }
    }

    let mut walked = termini.into_iter();
    let first = walked.next().unwrap_or_else(|| {
        Terminus::ExplicitAbsence(AbsenceState::NoRowReachesTheStartingPoint {
            direction,
            start: start.clone(),
        })
    });
    Termination {
        direction,
        start: start.clone(),
        first,
        rest: walked.collect(),
    }
}
