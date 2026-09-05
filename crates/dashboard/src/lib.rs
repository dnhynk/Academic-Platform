//! Sections 25.4, 25.5 and 25.6: the `Academic` branch of the section 25.1 tree.
//!
//! `P2-X1` fixed the frame — every route in that tree has a titled view with a
//! breadcrumb, at least one section and the evidence drawer, and each section
//! names the task that fills it. This crate is `P2-X3` filling the four it
//! named `P2-X3`: the academic dashboard, the semester planner, course detail,
//! and the graduation-audit view.
//!
//! # What this is not evidence for
//!
//! **No window opens.** `P2-X1` merged with no Tauri runtime linked and that
//! decision is still open under the user gate. Nothing here depends on a window
//! and nothing here is evidence that one exists: this crate is a set of typed
//! records and the rules between them, checked by compiling it, running its
//! tests, or reading its source. `packages/ui/src/academic.ts` is the shell
//! half, and it adds that opening `/academic/dashboard` yields sections naming
//! section 25.4's own lines instead of a promise that a later task will supply
//! some. That is a structure, not a rendering. `P2-X2`'s, `P2-X5`'s and
//! `P2-X7`'s pages say the same thing about their own.
//!
//! **No average is computed here.** [`GpaFigure`] carries a `P2-U4`
//! `academic_record::views::GpaValue` and the attempts that produced it. This
//! crate has no grading scheme, no repeat policy and no arithmetic over grades;
//! `dashboard_shows_three_gpas_with_proof` drives `academic-record`'s own
//! engine and compares what this surface publishes against what that engine
//! returned. A number this surface shows is a number that crate computed.
//!
//! **No verdict is computed here either.** [`AuditStateReading`] is a display
//! reading over a section 3.9 `academic_domain::engines::ProofStatus` a caller
//! supplies. There is no `academic-audit` and no `academic-requirement` product
//! edge, so no rule, no requirement set and no proof tree is nameable from a
//! product file in this crate.
//!
//! **The absence claims are about this crate's declared surface.** They are
//! whole-set statements over the items this crate compiles, not proofs that no
//! such path could ever be written. `crates/contracts/tests/item_inventory_scans.rs`
//! is where the workspace-wide half lives, and
//! `crates/dashboard/tests/dashboard_scans.rs` holds the ones that are about
//! this crate's own text.
//!
//! # Where the counts come from
//!
//! Nothing here asserts a count. Every enumeration is parsed out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` by the acceptance
//! suite and compared with the enumeration below in both directions and in
//! order:
//!
//! | enumeration | section 25's own text |
//! |---|---|
//! | [`GpaScope::ALL`] | `누적·학기·전공 GPA와 각 계산 proof.` |
//! | [`AuditState::ALL`] | 졸업 audit의 `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`. |
//! | [`LifecycleFacet::ALL`] | `수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.` |
//! | [`PlannerDimension::ALL`] | section 25.5's bullets after `다음을 즉시 재평가한다` |
//! | [`CourseSection::ALL`] | section 25.6's own block headings |
//! | [`CoverageTab::ALL`] | `DESIGNED / TAUGHT / PRACTICED / ASSESSED (겹치지 않는 탭)` |
//! | [`OpenGate::ALL`] | section 38.1's first six lines, plus section 38.2's seventh bullet |
//!
//! Six planning-versus-specification count mismatches were measured in this
//! run, one of them in section 25's own neighbourhood. This is the discipline
//! `P2-N3` and `P2-N6` set in response, and a seventh is recorded in
//! [`AuditState`]: **section 25.4's four display words are not the audit
//! engine's five statuses**, and the difference is written down rather than
//! resolved by inventing a fifth word or dropping a status.
//!
//! # It persists nothing
//!
//! No `academic-store` edge, no `academic-vault` edge, no migration number. It
//! reads no clock: every instant it holds arrived as an argument, which is also
//! why its tests can name the instants they assert against.

#![forbid(unsafe_code)]

mod audit_state;
mod course;
mod gate;
mod gpa;
mod percentage;
mod planner;
mod screen;
mod timeline;

pub use audit_state::{AuditState, AuditStateReading};
pub use course::{
    CatalogIdentity, Connections, CourseDetail, CourseSection, CoverageEntry, CoverageReport,
    CoverageTab, OfferingRow, ReviewSection,
};
pub use gate::OpenGate;
pub use gpa::{GpaFigure, GpaProof, GpaScope};
pub use percentage::{BreakdownPart, RequirementBreakdown, SecondaryPercentage};
pub use planner::{
    AxisReading, CandidateOffering, DragOutcome, MeetingSlot, PlanSnapshot, PlannerBoard,
    PlannerDimension, RequirementContribution, StaleInput, StaleMarking, WorkloadRange,
};
pub use screen::{AcademicDashboard, DashboardLine, DashboardSection};
pub use timeline::{AttemptTimeline, FacetReading, LifecycleFacet, TimelineEntry};

/// Everything these four surfaces refuse.
///
/// One variant per rule section 25 states. There is no catch-all arm: a refusal
/// this crate cannot name is a refusal it does not make.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DashboardError {
    /// Section 25.4's first line asks each average for its own proof.
    #[error("a {scope} average was published with no attempt behind it")]
    AverageWithoutProof {
        /// Which of section 25.4's three averages.
        scope: GpaScope,
    },
    /// The proof does not name the attempts the value says it is unknown for.
    #[error("a {scope} average is unknown for {missing} attempts its proof does not name")]
    ProofOmitsUnknownAttempts {
        /// Which of section 25.4's three averages.
        scope: GpaScope,
        /// How many named attempts the proof left out.
        missing: usize,
    },
    /// Section 25.4's last line: the breakdown is always attached.
    #[error("a percentage was offered with an empty breakdown")]
    PercentageWithoutBreakdown,
    /// One requirement, twice in one bar, is the merge section 25.4 warns about.
    #[error("the breakdown names the requirement {label} twice")]
    BreakdownRepeatsARequirement {
        /// The label that appeared more than once.
        label: String,
    },
    /// A bar over a part nobody can evaluate is a number made out of nothing.
    #[error("the breakdown holds {count} parts that are not evaluated, so no percentage is drawn")]
    PercentageOverAnUnsettledPart {
        /// How many parts read `UNKNOWN` or `CONFLICT`.
        count: usize,
    },
    /// A part whose requirement asks for nothing has no ratio.
    #[error("the requirement {label} requires no credits at all")]
    BreakdownPartRequiresNothing {
        /// The label that required nothing.
        label: String,
    },
    /// A part that counts more than its requirement asked for.
    #[error("the requirement {label} counts more credits than it requires")]
    BreakdownPartOverflows {
        /// The label that overflowed.
        label: String,
    },
    /// A field section 25 names cannot be empty text.
    #[error("a {0} was offered as empty text")]
    EmptyField(&'static str),
    /// Two placements of the same offering are one placement.
    #[error("the offering {0} is already on the board")]
    OfferingIsAlreadyPlaced(String),
    /// Section 25.5 saves a plan under a name; an unnamed plan is not a plan.
    #[error("a plan snapshot was saved with no label")]
    SnapshotWithoutLabel,
    /// A snapshot of nothing records no decision.
    #[error("a plan snapshot was saved with nothing on the board")]
    SnapshotOfAnEmptyBoard,
    /// A meeting slot whose end is not after its start bounds nothing.
    #[error("a meeting slot ends at minute {end} before it starts at minute {start}")]
    MeetingEndsBeforeItStarts {
        /// Minute of the week the slot starts at.
        start: u32,
        /// Minute of the week the slot claims to end at.
        end: u32,
    },
    /// Section 25.5's fifth line asks a workload for its range *and* its basis.
    #[error("a workload range was offered with no basis behind it")]
    WorkloadWithoutBasis,
    /// A range whose top is below its floor is not a range.
    #[error("a workload range runs from {low} to {high}")]
    WorkloadRangeIsInverted {
        /// The lower bound offered.
        low: u32,
        /// The upper bound offered.
        high: u32,
    },
    /// Section 25.6's coverage tabs partition the evidence, so an entry needs
    /// a predicate one of them answers for.
    #[error("{0} is not one of section 25.6's four coverage predicates")]
    PredicateIsNotACoverageTab(&'static str),
    /// A course detail with no offering row has no `Offerings` block.
    #[error("a course detail was assembled with no offering")]
    CourseDetailWithoutAnOffering,
}
