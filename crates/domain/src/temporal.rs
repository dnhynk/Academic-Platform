//! Bitemporal query coordinates, the named time-travel dimensions, and the
//! vocabulary that separates a change in the record from a change in how the
//! record is observed.
//!
//! # Both coordinates are one value
//!
//! ADR-003 forbids an ambiguous mutable "current" query: a caller that supplies
//! one coordinate has not said whether it wants the past as it was known then
//! or the past as it is understood now. [`TimeCoordinates`] therefore carries
//! both and offers no `Default`, no single-coordinate constructor, and no "now"
//! constructor, so a call site that has not decided both cannot build one.
//!
//! # Dimensions are the specification's list, not this crate's
//!
//! [`NAMED_TIME_TRAVEL_DIMENSIONS`] is the fifteen targets the design document
//! names in its temporal model, in the order it names them.
//! `time_travel_covers_all_fifteen_named_dimensions` reads that section out of
//! the committed design document and removes each [`TimeTravelDimension::spec_name`]
//! from it, so dropping an arm here leaves text behind and fails.
//!
//! # Observing a change is not the same as the change
//!
//! [`ChangeOrigin`] has four arms because the specification's temporal model
//! separates real growth from three kinds of change in the observation system:
//! identity merge/split, analyzer or model upgrade, and official-source
//! correction. The visualization section names a third transition kind, `user
//! scope change`, that is deliberately **not** an arm here: changing which
//! scope is displayed changes what a viewer is shown, not what the record says,
//! so it belongs to the view that owns the scope filter and never to a label on
//! canonical history.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::TimestampMillis;

/// The two coordinates every bitemporal read must supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeCoordinates {
    /// Replica-local acceptance sequence through which canonical facts are known.
    pub known_at_accept_seq: u64,
    /// Domain-valid instant at which canonical facts are evaluated.
    pub valid_at: TimestampMillis,
}

impl TimeCoordinates {
    /// Constructs explicit bitemporal coordinates.
    #[must_use]
    pub const fn new(known_at_accept_seq: u64, valid_at: TimestampMillis) -> Self {
        Self {
            known_at_accept_seq,
            valid_at,
        }
    }
}

/// One time-travel target named by the specification's temporal model.
///
/// The order is the specification's reading order, and [`Self::spec_bullet`]
/// records which of its seven bullets an arm came from, so a reviewer can put
/// any arm back against its source line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeTravelDimension {
    /// When a concept was first met at all.
    ConceptFirstExposure,
    /// When a concept was first exercised.
    FirstPractice,
    /// When a concept was first used to build or fix something.
    FirstApplication,
    /// How often a concept has recurred since.
    Repetition,
    /// How recently the evidence behind a concept's state was observed.
    Freshness,
    /// A question's create, reframe, partial, resolve, and reopen chain.
    QuestionChain,
    /// One recorded attempt at a course.
    CourseAttempt,
    /// One computed degree-audit version.
    DegreeAuditVersion,
    /// One immutable repository snapshot.
    ProjectSnapshot,
    /// The architecture observed in a project.
    Architecture,
    /// A finding and the classification it carries.
    FindingClassification,
    /// The role a user is currently interested in.
    RoleInterest,
    /// One published competency-bundle version.
    CompetencyBundleVersion,
    /// A blind spot's scope and how much of it is covered.
    BlindSpotScopeCoverage,
    /// A critical path's goal, cost, and route as they move.
    CriticalPathChange,
}

/// The fifteen named time-travel targets, in specification order.
pub const NAMED_TIME_TRAVEL_DIMENSIONS: [TimeTravelDimension; 15] = [
    TimeTravelDimension::ConceptFirstExposure,
    TimeTravelDimension::FirstPractice,
    TimeTravelDimension::FirstApplication,
    TimeTravelDimension::Repetition,
    TimeTravelDimension::Freshness,
    TimeTravelDimension::QuestionChain,
    TimeTravelDimension::CourseAttempt,
    TimeTravelDimension::DegreeAuditVersion,
    TimeTravelDimension::ProjectSnapshot,
    TimeTravelDimension::Architecture,
    TimeTravelDimension::FindingClassification,
    TimeTravelDimension::RoleInterest,
    TimeTravelDimension::CompetencyBundleVersion,
    TimeTravelDimension::BlindSpotScopeCoverage,
    TimeTravelDimension::CriticalPathChange,
];

impl TimeTravelDimension {
    /// Returns the stable wire discriminant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConceptFirstExposure => "CONCEPT_FIRST_EXPOSURE",
            Self::FirstPractice => "FIRST_PRACTICE",
            Self::FirstApplication => "FIRST_APPLICATION",
            Self::Repetition => "REPETITION",
            Self::Freshness => "FRESHNESS",
            Self::QuestionChain => "QUESTION_CHAIN",
            Self::CourseAttempt => "COURSE_ATTEMPT",
            Self::DegreeAuditVersion => "DEGREE_AUDIT_VERSION",
            Self::ProjectSnapshot => "PROJECT_SNAPSHOT",
            Self::Architecture => "ARCHITECTURE",
            Self::FindingClassification => "FINDING_CLASSIFICATION",
            Self::RoleInterest => "ROLE_INTEREST",
            Self::CompetencyBundleVersion => "COMPETENCY_BUNDLE_VERSION",
            Self::BlindSpotScopeCoverage => "BLIND_SPOT_SCOPE_COVERAGE",
            Self::CriticalPathChange => "CRITICAL_PATH_CHANGE",
        }
    }

    /// Returns the exact words the design document uses for this target.
    ///
    /// These are matched against the committed design document, so they are not
    /// paraphrases and may not be tidied.
    #[must_use]
    pub const fn spec_name(self) -> &'static str {
        match self {
            Self::ConceptFirstExposure => "concept first exposure",
            Self::FirstPractice => "first practice",
            Self::FirstApplication => "first application",
            Self::Repetition => "repetition",
            Self::Freshness => "freshness",
            Self::QuestionChain => "Question create/reframe/partial/resolve/reopen chain",
            Self::CourseAttempt => "CourseAttempt",
            Self::DegreeAuditVersion => "degree audit version",
            Self::ProjectSnapshot => "Project snapshot",
            Self::Architecture => "architecture",
            Self::FindingClassification => "finding/classification",
            Self::RoleInterest => "Role interest",
            Self::CompetencyBundleVersion => "competency bundle version",
            Self::BlindSpotScopeCoverage => "Blind Spot scope/coverage",
            Self::CriticalPathChange => "Critical Path의 목표·비용·경로 변화",
        }
    }

    /// Returns which of the temporal model's seven bullets named this target.
    #[must_use]
    pub const fn spec_bullet(self) -> u8 {
        match self {
            Self::ConceptFirstExposure
            | Self::FirstPractice
            | Self::FirstApplication
            | Self::Repetition
            | Self::Freshness => 1,
            Self::QuestionChain => 2,
            Self::CourseAttempt | Self::DegreeAuditVersion => 3,
            Self::ProjectSnapshot | Self::Architecture | Self::FindingClassification => 4,
            Self::RoleInterest | Self::CompetencyBundleVersion => 5,
            Self::BlindSpotScopeCoverage => 6,
            Self::CriticalPathChange => 7,
        }
    }

    /// Returns the canonical record this target is read from.
    ///
    /// Four targets have a landed carrier among the eighteen event schema v3
    /// registration arms. The other eleven have none yet, and the query surface
    /// refuses them rather than returning an empty page: an empty page reads as
    /// "nothing happened", which is a different statement from "this is not
    /// recorded yet".
    #[must_use]
    pub const fn carrier(self) -> DimensionCarrier {
        match self {
            Self::CourseAttempt => DimensionCarrier::Aggregate("ATTEMPT_RECORDED"),
            Self::DegreeAuditVersion => DimensionCarrier::Aggregate("AUDIT_COMPUTED"),
            Self::ProjectSnapshot => DimensionCarrier::Aggregate("SNAPSHOT_REGISTERED"),
            Self::FindingClassification => DimensionCarrier::Aggregate("FINDING_PUBLISHED"),
            Self::ConceptFirstExposure
            | Self::FirstPractice
            | Self::FirstApplication
            | Self::Repetition
            | Self::Freshness
            | Self::QuestionChain
            | Self::Architecture
            | Self::RoleInterest
            | Self::CompetencyBundleVersion
            | Self::BlindSpotScopeCoverage
            | Self::CriticalPathChange => DimensionCarrier::NotYetCarried,
        }
    }
}

/// Where a time-travel target's canonical record lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionCarrier {
    /// One of the eighteen event schema v3 registration arms, by wire discriminant.
    Aggregate(&'static str),
    /// No canonical carrier is landed for this target yet.
    NotYetCarried,
}

/// Why a projected value moved between two readings.
///
/// The four arms are the specification's separation of real growth from the
/// three ways the observation system itself can move. `user scope change` from
/// the visualization section is deliberately absent; see the module header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeOrigin {
    /// New or withdrawn evidence about the same identity, read by the same projector.
    EvidenceChange,
    /// An identity merge or split moved what the value is attached to.
    OntologyChange,
    /// The projector that computed the value changed version.
    AnalyzerUpgrade,
    /// An official source superseded an earlier official statement.
    OfficialSourceCorrection,
}

/// The four change origins, in stable order.
pub const CHANGE_ORIGINS: [ChangeOrigin; 4] = [
    ChangeOrigin::EvidenceChange,
    ChangeOrigin::OntologyChange,
    ChangeOrigin::AnalyzerUpgrade,
    ChangeOrigin::OfficialSourceCorrection,
];

impl ChangeOrigin {
    /// Returns the stable wire discriminant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceChange => "EVIDENCE_CHANGE",
            Self::OntologyChange => "ONTOLOGY_CHANGE",
            Self::AnalyzerUpgrade => "ANALYZER_UPGRADE",
            Self::OfficialSourceCorrection => "OFFICIAL_SOURCE_CORRECTION",
        }
    }

    /// Whether this origin is a change in the observation system rather than in
    /// what the user actually did.
    #[must_use]
    pub const fn is_observation_system_change(self) -> bool {
        !matches!(self, Self::EvidenceChange)
    }
}

/// What differed between two readings of one dimension.
///
/// Every field is an input the reader can hold fixed. A step that sets more
/// than one is not attributable to a single origin and is refused, which is
/// what forces the reader to split a known-time interval until each step is
/// origin-pure instead of guessing a precedence order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransitionCause {
    /// The projector version differed between the two readings.
    pub projector_changed: bool,
    /// An identity merge or split applied to the subject in the interval.
    pub identity_changed: bool,
    /// An official source superseded an earlier official statement in the interval.
    pub official_correction: bool,
    /// Any other canonical acceptance in the interval moved the value.
    pub other_evidence: bool,
}

impl TransitionCause {
    /// A step caused only by the projector changing version.
    #[must_use]
    pub const fn projector() -> Self {
        Self {
            projector_changed: true,
            identity_changed: false,
            official_correction: false,
            other_evidence: false,
        }
    }

    /// A step caused only by an identity merge or split.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            projector_changed: false,
            identity_changed: true,
            official_correction: false,
            other_evidence: false,
        }
    }

    /// A step caused only by an official-source correction.
    #[must_use]
    pub const fn official() -> Self {
        Self {
            projector_changed: false,
            identity_changed: false,
            official_correction: true,
            other_evidence: false,
        }
    }

    /// A step caused only by other evidence arriving or being withdrawn.
    #[must_use]
    pub const fn evidence() -> Self {
        Self {
            projector_changed: false,
            identity_changed: false,
            official_correction: false,
            other_evidence: true,
        }
    }

    /// Returns the origins this cause sets, in [`CHANGE_ORIGINS`] order.
    #[must_use]
    pub fn origins(self) -> Vec<ChangeOrigin> {
        let mut origins = Vec::with_capacity(CHANGE_ORIGINS.len());
        if self.other_evidence {
            origins.push(ChangeOrigin::EvidenceChange);
        }
        if self.identity_changed {
            origins.push(ChangeOrigin::OntologyChange);
        }
        if self.projector_changed {
            origins.push(ChangeOrigin::AnalyzerUpgrade);
        }
        if self.official_correction {
            origins.push(ChangeOrigin::OfficialSourceCorrection);
        }
        origins
    }

    /// Labels this cause when exactly one origin explains it.
    ///
    /// # Errors
    ///
    /// Returns [`TemporalError::UnexplainedTransition`] when nothing differed
    /// and [`TemporalError::AmbiguousOrigin`] when more than one input did.
    pub fn label(self) -> Result<ChangeOrigin, TemporalError> {
        let origins = self.origins();
        match origins.as_slice() {
            [] => Err(TemporalError::UnexplainedTransition),
            [single] => Ok(*single),
            many => Err(TemporalError::AmbiguousOrigin {
                origins: many
                    .iter()
                    .map(|origin| origin.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            }),
        }
    }
}

/// One reading of a dimension and what differed from the reading before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimensionStep {
    /// Coordinates this reading was taken at.
    pub at: TimeCoordinates,
    /// What differed from the previous reading.
    pub cause: TransitionCause,
}

/// One origin-pure step of an explained transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionSegment {
    /// The single origin that explains this step.
    pub origin: ChangeOrigin,
    /// Coordinates the step started from.
    pub from: TimeCoordinates,
    /// Coordinates the step ended at.
    pub to: TimeCoordinates,
}

/// One dimension's movement, decomposed until every step carries one origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainedTransition {
    /// The target whose value moved.
    pub dimension: TimeTravelDimension,
    /// Origin-pure steps in known-time order; empty when nothing moved.
    pub segments: Vec<TransitionSegment>,
}

impl ExplainedTransition {
    /// Whether every step in this transition is a change in the observation
    /// system rather than in what the user did.
    #[must_use]
    pub fn is_entirely_observation_system_change(&self) -> bool {
        !self.segments.is_empty()
            && self
                .segments
                .iter()
                .all(|segment| segment.origin.is_observation_system_change())
    }
}

/// Failures raised while labelling a transition.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TemporalError {
    /// A value moved with no origin-bearing input to explain it.
    #[error("transition has no origin-bearing cause; a value moved with nothing to explain it")]
    UnexplainedTransition,
    /// More than one origin-bearing input changed across the same step.
    #[error(
        "transition mixes more than one change origin ({origins}); \
         split the known-time interval until one remains"
    )]
    AmbiguousOrigin {
        /// The origins the step set, in [`CHANGE_ORIGINS`] order.
        origins: String,
    },
    /// A reading ran backwards in known time.
    #[error(
        "time-travel steps must not run backwards in known time: \
         {previous} is after {next}"
    )]
    UnorderedSteps {
        /// Known-time coordinate of the earlier step.
        previous: u64,
        /// Known-time coordinate of the later step.
        next: u64,
    },
    /// A dimension was queried that has no landed canonical carrier.
    #[error(
        "time-travel dimension {dimension} has no landed canonical carrier; \
         this is not an empty result"
    )]
    DimensionNotCarried {
        /// Wire discriminant of the refused dimension.
        dimension: &'static str,
    },
    /// A carried dimension was queried on a profile that holds no aggregates.
    ///
    /// The dimension has a carrier; this profile cannot hold one. Returning no
    /// rows would say the aggregate was never registered, which is a different
    /// statement.
    #[error(
        "time-travel dimension {dimension} is carried by {carrier}, which this \
         profile does not hold; this is not an empty result"
    )]
    AggregateLaneAbsent {
        /// Wire discriminant of the refused dimension.
        dimension: &'static str,
        /// Registration arm that would carry it.
        carrier: &'static str,
    },
}

/// Labels every step of one dimension's movement.
///
/// The caller supplies steps that are already origin-pure: it is the reader
/// that owns splitting a known-time interval at each origin-bearing acceptance
/// and holding the projector fixed across a recomputation. This function is
/// what refuses to guess when that splitting was not done.
///
/// # Errors
///
/// Returns [`TemporalError::UnorderedSteps`] when the steps run backwards in
/// known time, and propagates [`TransitionCause::label`]'s refusals otherwise.
pub fn explain_transition(
    dimension: TimeTravelDimension,
    origin_at: TimeCoordinates,
    steps: &[DimensionStep],
) -> Result<ExplainedTransition, TemporalError> {
    let mut segments = Vec::with_capacity(steps.len());
    let mut from = origin_at;
    for step in steps {
        if step.at.known_at_accept_seq < from.known_at_accept_seq {
            return Err(TemporalError::UnorderedSteps {
                previous: from.known_at_accept_seq,
                next: step.at.known_at_accept_seq,
            });
        }
        segments.push(TransitionSegment {
            origin: step.cause.label()?,
            from,
            to: step.at,
        });
        from = step.at;
    }
    Ok(ExplainedTransition {
        dimension,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::V3_EVENT_KINDS;

    fn at(known: u64, valid: i64) -> TimeCoordinates {
        TimeCoordinates::new(known, TimestampMillis::new(valid))
    }

    #[test]
    fn named_dimensions_are_unique_and_ordered_by_spec_bullet() {
        let mut discriminants: Vec<&str> = NAMED_TIME_TRAVEL_DIMENSIONS
            .iter()
            .map(|dimension| dimension.as_str())
            .collect();
        let count = discriminants.len();
        discriminants.sort_unstable();
        discriminants.dedup();
        assert_eq!(discriminants.len(), count);
        assert!(
            NAMED_TIME_TRAVEL_DIMENSIONS
                .windows(2)
                .all(|pair| pair[0].spec_bullet() <= pair[1].spec_bullet())
        );
        assert_eq!(NAMED_TIME_TRAVEL_DIMENSIONS[0].spec_bullet(), 1);
        assert_eq!(NAMED_TIME_TRAVEL_DIMENSIONS[14].spec_bullet(), 7);
    }

    #[test]
    fn every_declared_carrier_is_a_real_v3_registration_arm() {
        for dimension in NAMED_TIME_TRAVEL_DIMENSIONS {
            if let DimensionCarrier::Aggregate(kind) = dimension.carrier() {
                assert!(
                    V3_EVENT_KINDS.contains(&kind),
                    "{} names carrier {kind}, which is not a v3 registration arm",
                    dimension.as_str()
                );
            }
        }
    }

    #[test]
    fn a_single_cause_labels_and_a_mixed_one_refuses() {
        assert_eq!(
            TransitionCause::projector().label(),
            Ok(ChangeOrigin::AnalyzerUpgrade)
        );
        assert_eq!(
            TransitionCause::identity().label(),
            Ok(ChangeOrigin::OntologyChange)
        );
        assert_eq!(
            TransitionCause::official().label(),
            Ok(ChangeOrigin::OfficialSourceCorrection)
        );
        assert_eq!(
            TransitionCause::evidence().label(),
            Ok(ChangeOrigin::EvidenceChange)
        );
        assert_eq!(
            TransitionCause::default().label(),
            Err(TemporalError::UnexplainedTransition)
        );
        let mixed = TransitionCause {
            projector_changed: true,
            identity_changed: false,
            official_correction: true,
            other_evidence: false,
        };
        assert_eq!(
            mixed.label(),
            Err(TemporalError::AmbiguousOrigin {
                origins: "ANALYZER_UPGRADE, OFFICIAL_SOURCE_CORRECTION".to_owned(),
            })
        );
    }

    #[test]
    fn explaining_a_mixed_step_refuses_instead_of_choosing() {
        let mixed = DimensionStep {
            at: at(9, 200),
            cause: TransitionCause {
                projector_changed: true,
                identity_changed: true,
                official_correction: false,
                other_evidence: false,
            },
        };
        let refused =
            explain_transition(TimeTravelDimension::CourseAttempt, at(4, 200), &[mixed]).is_err();
        assert!(refused);
    }

    #[test]
    fn a_split_interval_yields_one_origin_per_segment() -> Result<(), TemporalError> {
        let explained = explain_transition(
            TimeTravelDimension::CourseAttempt,
            at(4, 200),
            &[
                DimensionStep {
                    at: at(7, 200),
                    cause: TransitionCause::identity(),
                },
                DimensionStep {
                    at: at(9, 200),
                    cause: TransitionCause::projector(),
                },
            ],
        )?;
        assert_eq!(explained.segments.len(), 2);
        assert_eq!(explained.segments[0].origin, ChangeOrigin::OntologyChange);
        assert_eq!(explained.segments[1].origin, ChangeOrigin::AnalyzerUpgrade);
        assert!(explained.is_entirely_observation_system_change());
        Ok(())
    }

    #[test]
    fn steps_may_not_run_backwards_in_known_time() {
        let backwards = explain_transition(
            TimeTravelDimension::Freshness,
            at(9, 200),
            &[DimensionStep {
                at: at(4, 200),
                cause: TransitionCause::evidence(),
            }],
        );
        assert_eq!(
            backwards,
            Err(TemporalError::UnorderedSteps {
                previous: 9,
                next: 4
            })
        );
    }
}
