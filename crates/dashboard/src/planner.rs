//! Section 25.5's three-pane planner: six axes on every drag, and no registration.
//!
//! > 좌측은 공식 개설 CourseOffering과 status, 가운데는 시간표, 우측은 scenario
//! > consequence다. 과목을 끌어놓으면 다음을 즉시 재평가한다.
//!
//! Six bullets follow that sentence and they are [`PlannerDimension::ALL`].
//! `planner_reevaluates_six_dimensions_on_drag` parses them out of the design
//! document and compares them with this enumeration in both directions and in
//! order.
//!
//! # Re-evaluated, not accumulated
//!
//! [`PlannerBoard::place`] and [`PlannerBoard::remove`] each return a **new**
//! board and a [`DragOutcome`] computed from the whole placed set. There is no
//! cache, no incremental update and no `&mut self` on the board, so a reading
//! cannot survive the placement that should have changed it. The strong half of
//! the acceptance test is not that six readings are present — that would pass
//! with six constants — but that placing a candidate and then removing it
//! returns an outcome equal to the one before, and that each of the six axes
//! moves for a candidate that carries a fact on it and only on it.
//!
//! # Section 25.5's last sentence is an absence
//!
//! > 사용자의 실제 수강신청을 자동 수행하지 않는다.
//!
//! This crate links `academic-record`, so `RegistrationConfirmation`,
//! `CourseAttempt` and `AttemptHistory` are all nameable from here. Nothing in
//! this module names any of them: a placement is a [`CandidateOffering`], a
//! saved plan is a [`PlanSnapshot`], and neither carries an `AttemptId` or an
//! `EvidenceId`, which are what `RegistrationConfirmation::new` requires and
//! refuses to be built without.
//!
//! That absence is checked two ways, and neither is a token list.
//! `planner_has_no_registration_endpoint` compares the **whole** set of type
//! names this module's items mention against a set that holds none of them, and
//! `every_item_that_reaches_a_closed_type_is_pinned` in
//! `crates/contracts/tests/item_inventory_scans.rs` pins the whole workspace's
//! items that reach `RegistrationConfirmation`, so a route added anywhere fails
//! by name. `P2-M4` already made confirming a registration non-delegable —
//! `RegistrationConfirmation::new` takes no actor, so no agent can be asked to
//! stand in for the user. The two claims are different: `P2-M4`'s is that
//! nobody may be delegated the act, and this one is that the planner has no
//! route to it at all.
//!
//! # A saved plan is immutable, and staleness is identified rather than applied
//!
//! > 안 A/B/C를 고정 snapshot으로 저장하고, 공식 정보가 바뀌면 무엇이
//! > stale해졌는지만 표시한다.
//!
//! [`PlanSnapshot`] has private fields, no setter, no `&mut` accessor and no
//! method taking `&mut self`. [`PlanSnapshot::restate`] returns a
//! [`StaleMarking`] and nothing else: it does not return an updated snapshot,
//! because *무엇이 stale해졌는지만* is the whole of what the sentence licenses.
//!
//! **`안 A/B/C` is an example and not a closed set.** The label is caller text
//! that has to be non-empty; there is no three-armed enumeration here, because
//! the specification names no third thing that a fourth scenario would violate.

use academic_curriculum::{CourseCode, TermCode};
use academic_domain::OfferingId;

use crate::DashboardError;

/// One of section 25.5's six re-evaluated axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlannerDimension {
    /// 학점, 충돌, 공식 prerequisite/restriction
    CreditsConflictsAndPrerequisites,
    /// 졸업 rule contribution proof
    GraduationRuleContribution,
    /// concept/competency exposure opportunity
    ConceptCompetencyExposure,
    /// 활성 project와 role relevance
    ProjectAndRoleRelevance,
    /// workload 범위·근거·편향
    WorkloadRangeBasisAndBias,
    /// 후속 course/path unlock
    FollowOnUnlock,
}

impl PlannerDimension {
    /// Every axis, in section 25.5's own order.
    pub const ALL: [Self; 6] = [
        Self::CreditsConflictsAndPrerequisites,
        Self::GraduationRuleContribution,
        Self::ConceptCompetencyExposure,
        Self::ProjectAndRoleRelevance,
        Self::WorkloadRangeBasisAndBias,
        Self::FollowOnUnlock,
    ];

    /// The bullet section 25.5 spells this axis with, verbatim.
    #[must_use]
    pub const fn spec_line(self) -> &'static str {
        match self {
            Self::CreditsConflictsAndPrerequisites => "학점, 충돌, 공식 prerequisite/restriction",
            Self::GraduationRuleContribution => "졸업 rule contribution proof",
            Self::ConceptCompetencyExposure => "concept/competency exposure opportunity",
            Self::ProjectAndRoleRelevance => "활성 project와 role relevance",
            Self::WorkloadRangeBasisAndBias => "workload 범위·근거·편향",
            Self::FollowOnUnlock => "후속 course/path unlock",
        }
    }

    /// The identifier `packages/ui`'s shell half shows this axis under.
    ///
    /// Written out rather than derived from the arm name, so a rename on one
    /// side fails against the other.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::CreditsConflictsAndPrerequisites => "CREDITS_CONFLICTS_AND_PREREQUISITES",
            Self::GraduationRuleContribution => "GRADUATION_RULE_CONTRIBUTION",
            Self::ConceptCompetencyExposure => "CONCEPT_COMPETENCY_EXPOSURE",
            Self::ProjectAndRoleRelevance => "PROJECT_AND_ROLE_RELEVANCE",
            Self::WorkloadRangeBasisAndBias => "WORKLOAD_RANGE_BASIS_AND_BIAS",
            Self::FollowOnUnlock => "FOLLOW_ON_UNLOCK",
        }
    }

    /// Section 25.5's own position for this axis, counting from one.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::CreditsConflictsAndPrerequisites => 1,
            Self::GraduationRuleContribution => 2,
            Self::ConceptCompetencyExposure => 3,
            Self::ProjectAndRoleRelevance => 4,
            Self::WorkloadRangeBasisAndBias => 5,
            Self::FollowOnUnlock => 6,
        }
    }

    /// The position in the outcome array this axis occupies.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A weekly meeting, as minutes from the start of the week.
///
/// The middle pane is a timetable and a conflict is an overlap, so the slot has
/// to be an interval rather than a label. Nothing here reads a clock: both
/// bounds arrive as arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeetingSlot {
    start: u32,
    end: u32,
}

impl MeetingSlot {
    /// Records a slot, refusing one that ends before it starts.
    pub const fn new(start: u32, end: u32) -> Result<Self, DashboardError> {
        if end <= start {
            return Err(DashboardError::MeetingEndsBeforeItStarts { start, end });
        }
        Ok(Self { start, end })
    }

    /// The minute of the week the slot starts at.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// The minute of the week the slot ends at.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }

    /// Whether two slots overlap. Touching is not overlapping.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// What one candidate contributes to one graduation rule, and the proof of it.
///
/// The proof reference is the identifier of the `P2-U3` proof node the
/// contribution was read off. This crate evaluates no rule; it carries the
/// reference so the right-hand pane can open the same node the audit shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementContribution {
    rule_label: String,
    credits: u32,
    proof_node: String,
}

impl RequirementContribution {
    /// Records one contribution and the proof node behind it.
    pub fn of(
        rule_label: impl Into<String>,
        credits: u32,
        proof_node: impl Into<String>,
    ) -> Result<Self, DashboardError> {
        let rule_label = rule_label.into();
        let proof_node = proof_node.into();
        if rule_label.trim().is_empty() {
            return Err(DashboardError::EmptyField("graduation rule label"));
        }
        if proof_node.trim().is_empty() {
            return Err(DashboardError::EmptyField("proof node reference"));
        }
        Ok(Self {
            rule_label,
            credits,
            proof_node,
        })
    }

    /// The rule the contribution is against.
    #[must_use]
    pub fn rule_label(&self) -> &str {
        &self.rule_label
    }

    /// How many credits it contributes.
    #[must_use]
    pub const fn credits(&self) -> u32 {
        self.credits
    }

    /// The proof node the contribution was read from.
    #[must_use]
    pub fn proof_node(&self) -> &str {
        &self.proof_node
    }
}

/// Section 25.5's fifth axis: a range, its basis, and its bias.
///
/// The constructor takes all three. A range with no basis is a number with no
/// provenance, and the sentence asks for `범위·근거·편향` together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadRange {
    low_hours: u32,
    high_hours: u32,
    basis: Vec<String>,
    bias: Vec<String>,
}

impl WorkloadRange {
    /// Records a workload range with the observations behind it.
    pub fn observed(
        low_hours: u32,
        high_hours: u32,
        basis: Vec<String>,
        bias: Vec<String>,
    ) -> Result<Self, DashboardError> {
        if high_hours < low_hours {
            return Err(DashboardError::WorkloadRangeIsInverted {
                low: low_hours,
                high: high_hours,
            });
        }
        if basis.is_empty() {
            return Err(DashboardError::WorkloadWithoutBasis);
        }
        Ok(Self {
            low_hours,
            high_hours,
            basis,
            bias,
        })
    }

    /// The floor of the observed range.
    #[must_use]
    pub const fn low_hours(&self) -> u32 {
        self.low_hours
    }

    /// The ceiling of the observed range.
    #[must_use]
    pub const fn high_hours(&self) -> u32 {
        self.high_hours
    }

    /// What the range was read from.
    #[must_use]
    pub fn basis(&self) -> &[String] {
        &self.basis
    }

    /// The warnings section 36.1 asks for — instructor and term differences.
    #[must_use]
    pub fn bias(&self) -> &[String] {
        &self.bias
    }
}

/// One official offering the left-hand pane can drag onto the timetable.
///
/// Every field is a fact the caller read from an official source. This crate
/// fetches nothing and forecasts nothing: `GATE-38-017` — 해당 학기의 최신
/// CourseOffering, 교수자, 정원, 시간표, syllabus, 평가 방식 — stays open, and
/// [`crate::OpenGate::CurrentTermOfferingFacts`] is where that is stated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateOffering {
    offering: OfferingId,
    course_code: CourseCode,
    term: TermCode,
    credits: u32,
    meeting: MeetingSlot,
    prerequisites: Vec<CourseCode>,
    contributions: Vec<RequirementContribution>,
    exposes: Vec<String>,
    relevant_to: Vec<String>,
    workload: WorkloadRange,
    unlocks: Vec<CourseCode>,
}

impl CandidateOffering {
    /// Records one candidate, with a fact on each of the six axes.
    ///
    /// Every axis is a parameter. There is no `Default` and no setter, so a
    /// candidate with a missing axis is unrepresentable rather than defaulted
    /// to empty — which would make that axis read the same for every candidate
    /// and the sixth reading a constant.
    #[expect(
        clippy::too_many_arguments,
        reason = "section 25.5 names six axes and a candidate carries a fact on each; \
                  grouping them into a struct of six fields would be the same list \
                  one indirection further from the sentence it comes from"
    )]
    #[must_use]
    pub fn declaring(
        offering: OfferingId,
        course_code: CourseCode,
        term: TermCode,
        credits: u32,
        meeting: MeetingSlot,
        prerequisites: Vec<CourseCode>,
        contributions: Vec<RequirementContribution>,
        exposes: Vec<String>,
        relevant_to: Vec<String>,
        workload: WorkloadRange,
        unlocks: Vec<CourseCode>,
    ) -> Self {
        Self {
            offering,
            course_code,
            term,
            credits,
            meeting,
            prerequisites,
            contributions,
            exposes,
            relevant_to,
            workload,
            unlocks,
        }
    }

    /// The offering identity.
    #[must_use]
    pub const fn offering(&self) -> OfferingId {
        self.offering
    }

    /// The course code the catalogue prints.
    #[must_use]
    pub const fn course_code(&self) -> &CourseCode {
        &self.course_code
    }

    /// The term the offering runs in.
    #[must_use]
    pub const fn term(&self) -> &TermCode {
        &self.term
    }

    /// The credits it carries.
    #[must_use]
    pub const fn credits(&self) -> u32 {
        self.credits
    }

    /// When it meets.
    #[must_use]
    pub const fn meeting(&self) -> MeetingSlot {
        self.meeting
    }

    /// The official prerequisites and restrictions.
    #[must_use]
    pub fn prerequisites(&self) -> &[CourseCode] {
        &self.prerequisites
    }

    /// What it contributes to the graduation rules, with proof references.
    #[must_use]
    pub fn contributions(&self) -> &[RequirementContribution] {
        &self.contributions
    }

    /// The concept and competency exposure opportunities it offers.
    #[must_use]
    pub fn exposes(&self) -> &[String] {
        &self.exposes
    }

    /// The active projects and roles it is relevant to.
    #[must_use]
    pub fn relevant_to(&self) -> &[String] {
        &self.relevant_to
    }

    /// Its observed workload range, basis and bias.
    #[must_use]
    pub const fn workload(&self) -> &WorkloadRange {
        &self.workload
    }

    /// The follow-on courses it unlocks.
    #[must_use]
    pub fn unlocks(&self) -> &[CourseCode] {
        &self.unlocks
    }
}

/// One axis's reading over the whole placed set.
///
/// Named `AxisReading` rather than `DimensionReading` because
/// `academic_review::DimensionReading` is a different thing — one reading of one
/// review dimension — and this crate links that crate, so both would be in
/// scope in `course.rs`. `academic-review`'s own
/// `no_login_bypass_or_evasion_module_exists` is what found the collision: it
/// pins the whole set of files outside that crate that name one of its public
/// types, and this file arrived in it on a name that had nothing to do with a
/// review.
///
/// The entries are the axis's own evidence, in placement order, so two boards
/// that differ on this axis have different readings and two that do not have
/// equal ones. There is no score: a number here would rank one axis's evidence
/// against another's, and section 25.5 asks for a consequence rather than a
/// ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisReading {
    dimension: PlannerDimension,
    entries: Vec<String>,
}

impl AxisReading {
    /// Which axis this reads.
    #[must_use]
    pub const fn dimension(&self) -> PlannerDimension {
        self.dimension
    }

    /// The evidence on this axis, in placement order.
    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

/// What a drag produced: one reading per axis, every time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragOutcome {
    readings: [AxisReading; PlannerDimension::ALL.len()],
}

impl DragOutcome {
    /// The six readings, in section 25.5's own order.
    #[must_use]
    pub const fn readings(&self) -> &[AxisReading; PlannerDimension::ALL.len()] {
        &self.readings
    }

    /// One axis's reading.
    #[must_use]
    pub const fn reading(&self, dimension: PlannerDimension) -> &AxisReading {
        &self.readings[dimension.index()]
    }
}

/// The middle pane: what is currently on the timetable.
///
/// Immutable. [`PlannerBoard::place`] and [`PlannerBoard::remove`] return a new
/// board, so an outcome and the board it was computed from cannot drift apart.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlannerBoard {
    placed: Vec<CandidateOffering>,
}

impl PlannerBoard {
    /// An empty board.
    #[must_use]
    pub const fn new() -> Self {
        Self { placed: Vec::new() }
    }

    /// What is on the board, in placement order.
    #[must_use]
    pub fn placed(&self) -> &[CandidateOffering] {
        &self.placed
    }

    /// Drags one candidate on, and re-evaluates all six axes.
    pub fn place(
        &self,
        candidate: CandidateOffering,
    ) -> Result<(Self, DragOutcome), DashboardError> {
        if self
            .placed
            .iter()
            .any(|existing| existing.offering() == candidate.offering())
        {
            return Err(DashboardError::OfferingIsAlreadyPlaced(
                candidate.offering().to_string(),
            ));
        }
        let mut placed = self.placed.clone();
        placed.push(candidate);
        let board = Self { placed };
        let outcome = board.evaluate();
        Ok((board, outcome))
    }

    /// Drags one candidate off, and re-evaluates all six axes.
    #[must_use]
    pub fn remove(&self, offering: OfferingId) -> (Self, DragOutcome) {
        let placed: Vec<CandidateOffering> = self
            .placed
            .iter()
            .filter(|existing| existing.offering() != offering)
            .cloned()
            .collect();
        let board = Self { placed };
        let outcome = board.evaluate();
        (board, outcome)
    }

    /// Re-evaluates every axis from the whole placed set.
    ///
    /// One pass per axis over the same slice. Nothing is carried over from a
    /// previous call, which is what makes `place` then `remove` return to the
    /// earlier outcome exactly.
    #[must_use]
    pub fn evaluate(&self) -> DragOutcome {
        let readings = PlannerDimension::ALL.map(|dimension| AxisReading {
            dimension,
            entries: self.entries_on(dimension),
        });
        DragOutcome { readings }
    }

    /// One axis's evidence over the whole board.
    fn entries_on(&self, dimension: PlannerDimension) -> Vec<String> {
        let mut entries: Vec<String> = Vec::new();
        match dimension {
            PlannerDimension::CreditsConflictsAndPrerequisites => {
                let total: u32 = self
                    .placed
                    .iter()
                    .fold(0, |sum, candidate| sum.saturating_add(candidate.credits()));
                entries.push(format!("credits/{total}"));
                for (index, candidate) in self.placed.iter().enumerate() {
                    for other in self.placed.iter().skip(index.saturating_add(1)) {
                        if candidate.meeting().overlaps(other.meeting()) {
                            entries.push(format!(
                                "conflict/{}/{}",
                                candidate.course_code().as_str(),
                                other.course_code().as_str()
                            ));
                        }
                    }
                    for prerequisite in candidate.prerequisites() {
                        entries.push(format!(
                            "prerequisite/{}/{}",
                            candidate.course_code().as_str(),
                            prerequisite.as_str()
                        ));
                    }
                }
            }
            PlannerDimension::GraduationRuleContribution => {
                for candidate in &self.placed {
                    for contribution in candidate.contributions() {
                        entries.push(format!(
                            "contribution/{}/{}/{}/{}",
                            candidate.course_code().as_str(),
                            contribution.rule_label(),
                            contribution.credits(),
                            contribution.proof_node()
                        ));
                    }
                }
            }
            PlannerDimension::ConceptCompetencyExposure => {
                for candidate in &self.placed {
                    for exposure in candidate.exposes() {
                        entries.push(format!(
                            "exposure/{}/{exposure}",
                            candidate.course_code().as_str()
                        ));
                    }
                }
            }
            PlannerDimension::ProjectAndRoleRelevance => {
                for candidate in &self.placed {
                    for relevance in candidate.relevant_to() {
                        entries.push(format!(
                            "relevance/{}/{relevance}",
                            candidate.course_code().as_str()
                        ));
                    }
                }
            }
            PlannerDimension::WorkloadRangeBasisAndBias => {
                let low: u32 = self.placed.iter().fold(0, |sum, candidate| {
                    sum.saturating_add(candidate.workload().low_hours())
                });
                let high: u32 = self.placed.iter().fold(0, |sum, candidate| {
                    sum.saturating_add(candidate.workload().high_hours())
                });
                entries.push(format!("workload/{low}-{high}"));
                for candidate in &self.placed {
                    for basis in candidate.workload().basis() {
                        entries.push(format!(
                            "basis/{}/{basis}",
                            candidate.course_code().as_str()
                        ));
                    }
                    for bias in candidate.workload().bias() {
                        entries.push(format!("bias/{}/{bias}", candidate.course_code().as_str()));
                    }
                }
            }
            PlannerDimension::FollowOnUnlock => {
                for candidate in &self.placed {
                    for unlocked in candidate.unlocks() {
                        entries.push(format!(
                            "unlock/{}/{}",
                            candidate.course_code().as_str(),
                            unlocked.as_str()
                        ));
                    }
                }
            }
        }
        entries
    }
}

/// One official input a saved plan rested on, which has since moved.
///
/// Each arm names the offering and what changed. None of them carries the new
/// value: *무엇이 stale해졌는지만 표시한다* is what the sentence licenses, and an
/// arm holding the replacement would be the first half of applying it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StaleInput {
    /// The offering is no longer in the official reading at all.
    OfferingIsGone(OfferingId),
    /// The offering's credits are not what the snapshot recorded.
    CreditsMoved(OfferingId),
    /// The offering's timetable slot is not what the snapshot recorded.
    MeetingMoved(OfferingId),
    /// The offering's official prerequisites are not what the snapshot recorded.
    PrerequisitesMoved(OfferingId),
}

/// Which of a snapshot's inputs went stale, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaleMarking {
    stale: Vec<StaleInput>,
}

impl StaleMarking {
    /// The stale inputs, in the snapshot's own placement order.
    #[must_use]
    pub fn stale(&self) -> &[StaleInput] {
        &self.stale
    }

    /// Whether anything went stale at all.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.stale.is_empty()
    }
}

/// A fixed scenario snapshot — 안 A, 안 B, 안 C, or any other label.
///
/// Every field is private, there is no setter, no `&mut` accessor and no method
/// taking `&mut self`. The only way to a snapshot is [`PlanSnapshot::save`],
/// and the only thing [`PlanSnapshot::restate`] returns is a [`StaleMarking`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSnapshot {
    label: String,
    placed: Vec<CandidateOffering>,
    outcome: DragOutcome,
}

impl PlanSnapshot {
    /// Fixes the board as a snapshot under a label.
    pub fn save(label: impl Into<String>, board: &PlannerBoard) -> Result<Self, DashboardError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(DashboardError::SnapshotWithoutLabel);
        }
        if board.placed().is_empty() {
            return Err(DashboardError::SnapshotOfAnEmptyBoard);
        }
        Ok(Self {
            label,
            placed: board.placed().to_vec(),
            outcome: board.evaluate(),
        })
    }

    /// The label the plan was saved under.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// What was on the board when the plan was fixed.
    #[must_use]
    pub fn placed(&self) -> &[CandidateOffering] {
        &self.placed
    }

    /// The six readings as they stood when the plan was fixed.
    #[must_use]
    pub const fn outcome(&self) -> &DragOutcome {
        &self.outcome
    }

    /// Says which of this snapshot's inputs the current official reading moved.
    ///
    /// Takes `&self` and returns only a marking. The snapshot is unchanged
    /// afterwards, and `plan_snapshot_is_immutable_and_stale_marked` compares
    /// the whole value before and after to say so rather than trusting the
    /// signature.
    #[must_use]
    pub fn restate(&self, official: &[CandidateOffering]) -> StaleMarking {
        let mut stale: Vec<StaleInput> = Vec::new();
        for recorded in &self.placed {
            let Some(current) = official
                .iter()
                .find(|candidate| candidate.offering() == recorded.offering())
            else {
                stale.push(StaleInput::OfferingIsGone(recorded.offering()));
                continue;
            };
            if current.credits() != recorded.credits() {
                stale.push(StaleInput::CreditsMoved(recorded.offering()));
            }
            if current.meeting() != recorded.meeting() {
                stale.push(StaleInput::MeetingMoved(recorded.offering()));
            }
            if current.prerequisites() != recorded.prerequisites() {
                stale.push(StaleInput::PrerequisitesMoved(recorded.offering()));
            }
        }
        StaleMarking { stale }
    }
}
