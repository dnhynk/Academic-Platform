//! Section 16.3's eight constraints, enumerated and each answered.
//!
//! ## Eight is a measurement
//!
//! `eight_constraints_are_enforced` reads section 16.3's own bullet list back
//! out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares it
//! against [`CONSTRAINTS`] in both directions, cell for cell. Eight is what the
//! design document lists.
//!
//! ## The eighth is the diagnostic checkpoint
//!
//! Section 16.3's last bullet is `불확실 edge가 일정 비율을 넘을 때 diagnostic
//! checkpoint 삽입`, which `t068` also names separately as
//! `uncertain_edge_ratio_inserts_diagnostic_checkpoint`. They are the same
//! rule: the eighth constraint's answer is [`crate::checkpoint`]'s decision, and
//! a reader who counted the two acceptance rows as two constraints would look
//! for a ninth bullet that does not exist. Recorded here so nobody adds one.
//!
//! ## The first and the second are different prerequisites
//!
//! Bullet one is `hard prerequisite satisfaction` -- a *concept* prerequisite,
//! `P2-C4`'s `REQUIRES` at `HARD`. Bullet two ends in `선수과목`, which is a
//! *course* prerequisite out of the official catalogue. §28's
//! `OFFICIAL_PREREQUISITE` engine's own invariant is `AI-inferred 선수지식과
//! 분리`, so this file keeps them as two constraints with two inputs, and
//! [`OfficialPrerequisiteStanding`] has an `Unknown` value because that engine
//! is `PLANNED` and folding its silence into a pass would manufacture a verdict.
//!
//! ## Every answer is produced, none is skipped
//!
//! [`evaluate`] returns `[ConstraintFinding; 8]` -- a fixed-size array, one
//! entry per [`CONSTRAINTS`] member in order. There is no filter, no `Option`
//! and no early return, so a plan cannot be produced with a constraint
//! unanswered.

use std::collections::BTreeSet;

use academic_curriculum::{Meeting, OfferingStatus, Weekday};
use academic_domain::{EntityId, FreshnessBand, OfferingId};
use serde::{Deserialize, Serialize};

use crate::{
    CriticalPathError,
    checkpoint::{CheckpointDecision, uncertain_edge_ratio_permille},
    hypergraph::SatisfyingSet,
    option::AcquisitionOption,
};

/// Section 16.3's eight constraints, in its own bullet order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Constraint {
    /// `hard prerequisite satisfaction`.
    HardPrerequisiteSatisfaction,
    /// `현재/미래 CourseOffering의 확인 상태와 선수과목`.
    OfferingStandingAndOfficialPrerequisite,
    /// `학기 시간표·학점 한도`.
    TimetableAndCreditLimit,
    /// `project deadline 또는 목표 horizon`.
    DeadlineOrHorizon,
    /// `privacy상 사용할 수 없는 provider/resource`.
    PrivacyExcludedResource,
    /// `사용자가 제외한 분야·과목·학습 방식`.
    UserExclusion,
    /// `stale concept의 최소 refresh requirement`.
    StaleRefreshRequirement,
    /// `불확실 edge가 일정 비율을 넘을 때 diagnostic checkpoint 삽입`.
    UncertainEdgeCheckpoint,
}

/// The eight, in section 16.3's own bullet order.
pub const CONSTRAINTS: [Constraint; 8] = [
    Constraint::HardPrerequisiteSatisfaction,
    Constraint::OfferingStandingAndOfficialPrerequisite,
    Constraint::TimetableAndCreditLimit,
    Constraint::DeadlineOrHorizon,
    Constraint::PrivacyExcludedResource,
    Constraint::UserExclusion,
    Constraint::StaleRefreshRequirement,
    Constraint::UncertainEdgeCheckpoint,
];

impl Constraint {
    /// Section 16.3's bullet text for this constraint, verbatim.
    #[must_use]
    pub const fn spec_bullet(self) -> &'static str {
        match self {
            Self::HardPrerequisiteSatisfaction => "hard prerequisite satisfaction",
            Self::OfferingStandingAndOfficialPrerequisite => {
                "현재/미래 CourseOffering의 확인 상태와 선수과목"
            }
            Self::TimetableAndCreditLimit => "학기 시간표·학점 한도",
            Self::DeadlineOrHorizon => "project deadline 또는 목표 horizon",
            Self::PrivacyExcludedResource => "privacy상 사용할 수 없는 provider/resource",
            Self::UserExclusion => "사용자가 제외한 분야·과목·학습 방식",
            Self::StaleRefreshRequirement => "stale concept의 최소 refresh requirement",
            Self::UncertainEdgeCheckpoint => {
                "불확실 edge가 일정 비율을 넘을 때 diagnostic checkpoint 삽입"
            }
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HardPrerequisiteSatisfaction => "HARD_PREREQUISITE_SATISFACTION",
            Self::OfferingStandingAndOfficialPrerequisite => {
                "OFFERING_STANDING_AND_OFFICIAL_PREREQUISITE"
            }
            Self::TimetableAndCreditLimit => "TIMETABLE_AND_CREDIT_LIMIT",
            Self::DeadlineOrHorizon => "DEADLINE_OR_HORIZON",
            Self::PrivacyExcludedResource => "PRIVACY_EXCLUDED_RESOURCE",
            Self::UserExclusion => "USER_EXCLUSION",
            Self::StaleRefreshRequirement => "STALE_REFRESH_REQUIREMENT",
            Self::UncertainEdgeCheckpoint => "UNCERTAIN_EDGE_CHECKPOINT",
        }
    }
}

/// What one constraint concluded about one candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConstraintVerdict {
    /// The candidate meets the constraint.
    Satisfied,
    /// The candidate violates it and is not a plan.
    Violated,
    /// The candidate meets it once a required step is inserted. The step is on
    /// the finding.
    SatisfiedWithInsertion,
    /// The input that decides this constraint was itself unknown.
    ///
    /// A distinct value on purpose: §28's `Graduation Audit` invariant is
    /// `unknown을 pass/fail로 강제하지 않음`, and this is the same refusal in
    /// this engine. An `Unknown` candidate is disclosed and is not silently
    /// admitted or silently dropped.
    Unknown,
}

impl ConstraintVerdict {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "SATISFIED",
            Self::Violated => "VIOLATED",
            Self::SatisfiedWithInsertion => "SATISFIED_WITH_INSERTION",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Whether a candidate carrying this verdict may still be ranked.
    #[must_use]
    pub const fn admits(self) -> bool {
        match self {
            Self::Satisfied | Self::SatisfiedWithInsertion => true,
            Self::Violated | Self::Unknown => false,
        }
    }
}

/// What the official catalogue says about a course prerequisite.
///
/// [`OfficialPrerequisiteStanding::Unknown`] is a value, not a missing answer.
/// §28's `OFFICIAL_PREREQUISITE` engine is `PLANNED`, so nobody has computed it
/// yet, and folding that silence into `Met` is how a plan recommends a course
/// the registrar refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OfficialPrerequisiteStanding {
    /// The registrar's prerequisites for this offering are met.
    Met,
    /// They are not met.
    Unmet,
    /// Nobody has evaluated them.
    Unknown,
}

/// Which step a constraint requires to be inserted before the path is walked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "insertion", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RequiredInsertion {
    /// Section 16.3's seventh bullet: a stale concept's minimum refresh.
    MinimumRefresh {
        /// Which concepts are stale, in identifier order.
        concepts: Vec<EntityId>,
    },
    /// Section 16.3's eighth bullet: a diagnostic checkpoint.
    DiagnosticCheckpoint {
        /// The measured uncertain-edge ratio, in permille.
        ratio_permille: u16,
    },
}

/// One constraint's answer about one candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintFinding {
    constraint: Constraint,
    verdict: ConstraintVerdict,
    subjects: Vec<EntityId>,
    insertion: Option<RequiredInsertion>,
}

impl ConstraintFinding {
    /// Records one answer.
    #[must_use]
    pub fn of(
        constraint: Constraint,
        verdict: ConstraintVerdict,
        subjects: Vec<EntityId>,
        insertion: Option<RequiredInsertion>,
    ) -> Self {
        let mut ordered = subjects;
        ordered.sort_by_key(|id| id.as_uuid());
        ordered.dedup();
        Self {
            constraint,
            verdict,
            subjects: ordered,
            insertion,
        }
    }

    /// Which constraint.
    #[must_use]
    pub const fn constraint(&self) -> Constraint {
        self.constraint
    }

    /// What it concluded.
    #[must_use]
    pub const fn verdict(&self) -> ConstraintVerdict {
        self.verdict
    }

    /// The concepts the verdict is about, in identifier order.
    #[must_use]
    pub fn subjects(&self) -> &[EntityId] {
        &self.subjects
    }

    /// The step the constraint requires, when it requires one.
    #[must_use]
    pub const fn insertion(&self) -> Option<&RequiredInsertion> {
        self.insertion.as_ref()
    }
}

/// Everything the eight constraints read, supplied by the caller.
///
/// Every field is a fact somebody else decided. This engine evaluates; it does
/// not decide whether an offering runs, what the registrar requires, how many
/// credits a term admits, when a deadline is, which provider privacy allows,
/// what the user excluded, or which concept is stale.
#[derive(Debug, Clone, Default)]
pub struct ConstraintInputs {
    /// Concepts whose `HARD` prerequisite the user already meets. Bullet one.
    pub hard_prerequisites_met: Vec<EntityId>,
    /// The registrar's answer for each offering the candidate uses. Bullet two.
    pub official_prerequisites: Vec<(OfferingId, OfficialPrerequisiteStanding)>,
    /// Meetings already committed this term. Bullet three.
    pub committed_meetings: Vec<Meeting>,
    /// Meetings each offering the candidate uses would add. Bullet three.
    pub offering_meetings: Vec<(OfferingId, Vec<Meeting>)>,
    /// Credits already committed this term. Bullet three.
    pub committed_credits: u8,
    /// The term's credit ceiling. Bullet three.
    pub credit_limit: u8,
    /// Days between now and the goal's deadline or horizon. Bullet four.
    pub horizon_days: u32,
    /// Evidence sources privacy forbids reaching. Bullet five.
    pub privacy_excluded_sources: Vec<academic_domain::EvidenceId>,
    /// Fields, courses and study methods the user excluded. Bullet six.
    pub user_excluded_concepts: Vec<EntityId>,
    /// Offerings the user excluded. Bullet six.
    pub user_excluded_offerings: Vec<OfferingId>,
    /// Each concept's `P2-N3` band. Bullet seven.
    pub bands: Vec<(EntityId, FreshnessBand)>,
}

/// The band section 16.3's seventh bullet calls `stale`.
///
/// **Exactly one**, and it is the band of that name. Section 13.3's own gloss
/// is `STALE: 과거 evidence는 있으나 최근성 낮음` and section 15.2's table row is
/// `mastery evidence는 있으나 stale`, so a refresh has something to refresh.
///
/// `UNKNOWN` is deliberately **not** stale. `P2-N3`'s own module note says it
/// `is not "very stale": it is the band for a concept about which nothing
/// datable was ever admitted`, and inserting a refresh requirement for a
/// concept with no record is a step the user cannot perform. That concept's
/// answer is `P2-N5`'s evidence gap, not this constraint.
///
/// Total over `academic_domain::FreshnessBand` with no wildcard arm, so a
/// seventh band is a compile error rather than a silent admission.
#[must_use]
pub const fn is_stale(band: FreshnessBand) -> bool {
    match band {
        FreshnessBand::Stale => true,
        FreshnessBand::Unknown
        | FreshnessBand::Low
        | FreshnessBand::Moderate
        | FreshnessBand::High
        | FreshnessBand::VeryHigh => false,
    }
}

/// Answers all eight constraints for one candidate.
///
/// Returns one finding per [`CONSTRAINTS`] member, in order, always eight.
///
/// # Errors
///
/// [`CriticalPathError::ConstraintCountChanged`] if the array assembled here
/// stops being the length of [`CONSTRAINTS`], which can only happen if a
/// constraint is added without adding its answer.
pub fn evaluate(
    set: &SatisfyingSet,
    options: &[AcquisitionOption],
    inputs: &ConstraintInputs,
    calendar_delay_days_high: u32,
) -> Result<[ConstraintFinding; 8], CriticalPathError> {
    let findings = vec![
        hard_prerequisites(set, inputs),
        offering_standing(options, inputs),
        timetable_and_credits(options, inputs),
        deadline_or_horizon(inputs, calendar_delay_days_high),
        privacy_excluded(options, inputs),
        user_exclusions(set, options, inputs),
        stale_refresh(set, inputs),
        uncertain_checkpoint(set),
    ];
    findings
        .try_into()
        .map_err(|_| CriticalPathError::ConstraintCountChanged)
}

fn hard_prerequisites(set: &SatisfyingSet, inputs: &ConstraintInputs) -> ConstraintFinding {
    let met: BTreeSet<[u8; 16]> = inputs
        .hard_prerequisites_met
        .iter()
        .map(|id| *id.as_bytes())
        .collect();
    let unmet: Vec<EntityId> = set
        .members()
        .iter()
        .filter(|member| member.edge().floor().is_some())
        .map(|member| member.dependent())
        .filter(|dependent| !met.contains(dependent.as_bytes()))
        .collect();
    // A blocking edge whose dependent has no recorded satisfaction is exactly
    // what the plan is *for*: the set is the remediation, so the constraint is
    // satisfied by the set existing. It is violated only when the set requires
    // a concept the goal cannot reach at all, which is an empty set.
    let verdict = if set.concepts().is_empty() {
        ConstraintVerdict::Violated
    } else {
        ConstraintVerdict::Satisfied
    };
    ConstraintFinding::of(
        Constraint::HardPrerequisiteSatisfaction,
        verdict,
        unmet,
        None,
    )
}

fn offering_standing(
    options: &[AcquisitionOption],
    inputs: &ConstraintInputs,
) -> ConstraintFinding {
    let mut verdict = ConstraintVerdict::Satisfied;
    for option in options {
        let Some(offering) = option.offering() else {
            continue;
        };
        // Section 8.3's own four values. `Cancelled` is a refusal; `Uncertain`
        // and `HistoricallyLikely` are not confirmations, and reading either as
        // one is what section 36.7 calls confusing an official course with an
        // offering's actual coverage.
        let standing = match option.offering_status() {
            Some(OfferingStatus::Confirmed) => ConstraintVerdict::Satisfied,
            Some(OfferingStatus::Cancelled) => ConstraintVerdict::Violated,
            Some(OfferingStatus::HistoricallyLikely | OfferingStatus::Uncertain) | None => {
                ConstraintVerdict::Unknown
            }
        };
        verdict = worse(verdict, standing);
        let official = inputs
            .official_prerequisites
            .iter()
            .find(|(candidate, _)| *candidate == offering)
            .map_or(OfficialPrerequisiteStanding::Unknown, |(_, value)| *value);
        verdict = worse(
            verdict,
            match official {
                OfficialPrerequisiteStanding::Met => ConstraintVerdict::Satisfied,
                OfficialPrerequisiteStanding::Unmet => ConstraintVerdict::Violated,
                OfficialPrerequisiteStanding::Unknown => ConstraintVerdict::Unknown,
            },
        );
    }
    ConstraintFinding::of(
        Constraint::OfferingStandingAndOfficialPrerequisite,
        verdict,
        Vec::new(),
        None,
    )
}

fn timetable_and_credits(
    options: &[AcquisitionOption],
    inputs: &ConstraintInputs,
) -> ConstraintFinding {
    let mut planned: Vec<Meeting> = inputs.committed_meetings.clone();
    let mut credits: u32 = u32::from(inputs.committed_credits);
    for option in options {
        credits = credits.saturating_add(u32::from(option.credits()));
        if let Some(offering) = option.offering()
            && let Some((_, meetings)) = inputs
                .offering_meetings
                .iter()
                .find(|(candidate, _)| *candidate == offering)
        {
            planned.extend(meetings.iter().copied());
        }
    }
    let overlapping = planned.iter().enumerate().any(|(index, left)| {
        planned
            .iter()
            .skip(index + 1)
            .any(|right| meetings_overlap(*left, *right))
    });
    let verdict = if overlapping || credits > u32::from(inputs.credit_limit) {
        ConstraintVerdict::Violated
    } else {
        ConstraintVerdict::Satisfied
    };
    ConstraintFinding::of(
        Constraint::TimetableAndCreditLimit,
        verdict,
        Vec::new(),
        None,
    )
}

/// Two meetings share a minute of the same weekday.
///
/// Half-open, so a class ending at the minute the next begins does not clash.
/// `Weekday` is `P2-U1`'s enumeration and the comparison is on it directly.
fn meetings_overlap(left: Meeting, right: Meeting) -> bool {
    let same_day: fn(Weekday, Weekday) -> bool = |a, b| a == b;
    same_day(left.weekday(), right.weekday())
        && left.from_minute() < right.to_minute()
        && right.from_minute() < left.to_minute()
}

fn deadline_or_horizon(inputs: &ConstraintInputs, delay_days_high: u32) -> ConstraintFinding {
    // The *high* end of the calendar-delay interval is what has to fit. Using
    // the low end would admit a path that fits only if every unknown resolves
    // favourably, which is the false precision section 16.2 refuses.
    let verdict = if delay_days_high > inputs.horizon_days {
        ConstraintVerdict::Violated
    } else {
        ConstraintVerdict::Satisfied
    };
    ConstraintFinding::of(Constraint::DeadlineOrHorizon, verdict, Vec::new(), None)
}

fn privacy_excluded(options: &[AcquisitionOption], inputs: &ConstraintInputs) -> ConstraintFinding {
    let forbidden: BTreeSet<[u8; 16]> = inputs
        .privacy_excluded_sources
        .iter()
        .map(|id| *id.as_bytes())
        .collect();
    let hit: Vec<EntityId> = options
        .iter()
        .flat_map(AcquisitionOption::supplies)
        .filter(|opportunity| forbidden.contains(opportunity.source().as_bytes()))
        .map(crate::option::Opportunity::concept)
        .collect();
    let verdict = if hit.is_empty() {
        ConstraintVerdict::Satisfied
    } else {
        ConstraintVerdict::Violated
    };
    ConstraintFinding::of(Constraint::PrivacyExcludedResource, verdict, hit, None)
}

fn user_exclusions(
    set: &SatisfyingSet,
    options: &[AcquisitionOption],
    inputs: &ConstraintInputs,
) -> ConstraintFinding {
    let excluded: BTreeSet<[u8; 16]> = inputs
        .user_excluded_concepts
        .iter()
        .map(|id| *id.as_bytes())
        .collect();
    let mut hit: Vec<EntityId> = set
        .concepts()
        .iter()
        .copied()
        .filter(|concept| excluded.contains(concept.as_bytes()))
        .collect();
    let offering_hit = options.iter().any(|option| {
        option
            .offering()
            .is_some_and(|offering| inputs.user_excluded_offerings.contains(&offering))
    });
    if offering_hit {
        hit.extend(
            options
                .iter()
                .filter(|option| {
                    option
                        .offering()
                        .is_some_and(|offering| inputs.user_excluded_offerings.contains(&offering))
                })
                .flat_map(|option| {
                    option
                        .supplies()
                        .iter()
                        .map(crate::option::Opportunity::concept)
                }),
        );
    }
    let verdict = if hit.is_empty() {
        ConstraintVerdict::Satisfied
    } else {
        ConstraintVerdict::Violated
    };
    ConstraintFinding::of(Constraint::UserExclusion, verdict, hit, None)
}

fn stale_refresh(set: &SatisfyingSet, inputs: &ConstraintInputs) -> ConstraintFinding {
    let stale: Vec<EntityId> = set
        .concepts()
        .iter()
        .copied()
        .filter(|concept| {
            inputs
                .bands
                .iter()
                .any(|(candidate, band)| candidate == concept && is_stale(*band))
        })
        .collect();
    if stale.is_empty() {
        return ConstraintFinding::of(
            Constraint::StaleRefreshRequirement,
            ConstraintVerdict::Satisfied,
            Vec::new(),
            None,
        );
    }
    let mut ordered = stale.clone();
    ordered.sort_by_key(|id| id.as_uuid());
    ordered.dedup();
    ConstraintFinding::of(
        Constraint::StaleRefreshRequirement,
        ConstraintVerdict::SatisfiedWithInsertion,
        stale,
        Some(RequiredInsertion::MinimumRefresh { concepts: ordered }),
    )
}

fn uncertain_checkpoint(set: &SatisfyingSet) -> ConstraintFinding {
    let ratio = uncertain_edge_ratio_permille(set);
    match CheckpointDecision::for_ratio(ratio) {
        CheckpointDecision::BelowThreshold => ConstraintFinding::of(
            Constraint::UncertainEdgeCheckpoint,
            ConstraintVerdict::Satisfied,
            Vec::new(),
            None,
        ),
        CheckpointDecision::Insert => ConstraintFinding::of(
            Constraint::UncertainEdgeCheckpoint,
            ConstraintVerdict::SatisfiedWithInsertion,
            set.members()
                .iter()
                .filter(|member| member.standing() == crate::hypergraph::EdgeStanding::Uncertain)
                .map(crate::hypergraph::EdgeMember::concept)
                .collect(),
            Some(RequiredInsertion::DiagnosticCheckpoint {
                ratio_permille: ratio,
            }),
        ),
    }
}

/// The stricter of two verdicts.
///
/// Total over both arguments with no wildcard arm. `Violated` beats `Unknown`
/// beats `SatisfiedWithInsertion` beats `Satisfied`: a candidate that is
/// refused for one reason is refused, and one whose input nobody has decided is
/// not upgraded by another input that was decided.
const fn worse(left: ConstraintVerdict, right: ConstraintVerdict) -> ConstraintVerdict {
    match (left, right) {
        (ConstraintVerdict::Violated, _) | (_, ConstraintVerdict::Violated) => {
            ConstraintVerdict::Violated
        }
        (ConstraintVerdict::Unknown, _) | (_, ConstraintVerdict::Unknown) => {
            ConstraintVerdict::Unknown
        }
        (ConstraintVerdict::SatisfiedWithInsertion, _)
        | (_, ConstraintVerdict::SatisfiedWithInsertion) => {
            ConstraintVerdict::SatisfiedWithInsertion
        }
        (ConstraintVerdict::Satisfied, ConstraintVerdict::Satisfied) => {
            ConstraintVerdict::Satisfied
        }
    }
}
