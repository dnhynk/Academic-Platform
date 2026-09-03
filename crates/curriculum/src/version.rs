//! Section 8.2's `CurriculumVersion`, and the transitional measure that sits
//! beside it rather than inside a course relation.
//!
//! # Why 경과조치 is here and not in [`crate::relation`]
//!
//! Section 11.4 makes 경과조치 an independent rule beside 동일, 대체 and 폐지.
//! Section 8.1 says where it applies: *2026학번 전공 표준형태는 2026학번부터
//! 적용하며 2025학번 이전은 종전 형태 적용*, tabulated as *version
//! applicability + transition rule*. The unit that moves is an admission
//! cohort and the thing it moves between is two curriculum versions, so a
//! transitional measure is not a statement about a pair of courses and cannot
//! be one: a course relation has two course ends and nowhere to put a cohort.
//!
//! `t068` section 5's `P2-U1` entry lists five names — identity, equivalency,
//! replacement, retirement, transition — and calls them *four* relations. The
//! specification's own sentence names four (동일·대체·폐지·경과조치) and puts
//! the fourth at this level. The placement follows the specification; the
//! divergence is recorded in `docs/contracts/curriculum-aggregates.md`.
//!
//! # `supersedes` and a transition are two different facts
//!
//! Section 8.2 gives `CurriculumVersion` a `supersedes` field. That says which
//! version this one follows. It does not say which admission cohorts move to
//! it, and section 8.1's row is exactly the case where the two differ: the 2026
//! standard supersedes the 2025 standard *and* leaves cohorts before 2026 on
//! the earlier form. So [`CurriculumVersion::supersedes`] and
//! [`TransitionArrangement`] are separate values, a version may supersede
//! another while recording no arrangement, and
//! [`CurriculumVersion::transition_for`] returns
//! [`CohortTransition::Unknown`] when it does.
//!
//! A `DegreeRequirementSet` also carries `transitionRules` (section 11.1).
//! That is `P2-U2`'s aggregate and its own field; nothing here is that.

use academic_domain::{ContentDigest, CurriculumVersionId, ValidInterval};

use crate::{error::CurriculumError, text::AdmissionCohort};

/// What a curriculum version's own publication status is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PublicationStatus {
    /// Confirmed against an official source snapshot.
    OfficialConfirmed,
    /// Recorded but not yet confirmed against an official source.
    Unknown,
}

impl PublicationStatus {
    /// Exhaustive listing.
    pub const ALL: [Self; 2] = [Self::OfficialConfirmed, Self::Unknown];

    /// Stable spelling, which is also migration 0014's `CHECK` vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfficialConfirmed => "OFFICIAL_CONFIRMED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// What a recorded [`TransitionArrangement`] says about one admission cohort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CohortTransition {
    /// No arrangement addresses this cohort. Nothing is inferred from
    /// `supersedes`, from the effective dates, or from the cohort number.
    Unknown,
    /// The cohort moves to this version.
    Moves,
    /// The cohort stays on the version this one supersedes (종전 형태 적용).
    Stays,
}

impl CohortTransition {
    /// Exhaustive listing, `Unknown` first.
    pub const ALL: [Self; 3] = [Self::Unknown, Self::Moves, Self::Stays];

    /// Stable spelling, which is also migration 0014's `CHECK` vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Moves => "MOVES",
            Self::Stays => "STAYS",
        }
    }
}

/// 경과조치: one recorded arrangement for one admission cohort.
///
/// It names a cohort and a disposition. It names no course, which is what makes
/// it unreachable from the three course-level relations and them from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionArrangement {
    cohort: AdmissionCohort,
    disposition: CohortTransition,
    valid_time: ValidInterval,
}

impl TransitionArrangement {
    /// Records one arrangement.
    ///
    /// `disposition` may not be [`CohortTransition::Unknown`], for the reason
    /// [`crate::relation::IdentityDecision::record`] gives: `Unknown` is the
    /// absence of a record.
    pub fn record(
        cohort: AdmissionCohort,
        disposition: CohortTransition,
        valid_time: ValidInterval,
    ) -> Result<Self, CurriculumError> {
        if matches!(disposition, CohortTransition::Unknown) {
            return Err(CurriculumError::Malformed {
                field: "transition arrangement",
                reason: "UNKNOWN is the absence of an arrangement, not one that can be recorded",
            });
        }
        Ok(Self {
            cohort,
            disposition,
            valid_time,
        })
    }

    /// Which admission cohort.
    #[must_use]
    pub const fn cohort(&self) -> &AdmissionCohort {
        &self.cohort
    }

    /// What the arrangement says.
    #[must_use]
    pub const fn disposition(&self) -> CohortTransition {
        self.disposition
    }

    /// When the arrangement applies.
    #[must_use]
    pub const fn valid_time(&self) -> ValidInterval {
        self.valid_time
    }
}

/// Section 8.2's `CurriculumVersion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurriculumVersion {
    id: CurriculumVersionId,
    institution_path: Vec<String>,
    admission_year_range: (AdmissionCohort, AdmissionCohort),
    status: PublicationStatus,
    source_snapshot: Option<ContentDigest>,
    supersedes: Option<CurriculumVersionId>,
    transitions: Vec<TransitionArrangement>,
    valid_time: ValidInterval,
}

impl CurriculumVersion {
    /// The version identifier.
    #[must_use]
    pub const fn id(&self) -> CurriculumVersionId {
        self.id
    }

    /// Section 8.2's `institutionPath`.
    #[must_use]
    pub fn institution_path(&self) -> &[String] {
        &self.institution_path
    }

    /// Section 8.2's `admissionYearRange`, inclusive on both ends.
    #[must_use]
    pub const fn admission_year_range(&self) -> (&AdmissionCohort, &AdmissionCohort) {
        (&self.admission_year_range.0, &self.admission_year_range.1)
    }

    /// Section 8.2's `status`.
    #[must_use]
    pub const fn status(&self) -> PublicationStatus {
        self.status
    }

    /// Section 8.2's `sourceSnapshot`.
    #[must_use]
    pub const fn source_snapshot(&self) -> Option<&ContentDigest> {
        self.source_snapshot.as_ref()
    }

    /// Section 8.2's `supersedes`. Which version this one follows, and nothing
    /// about which cohorts move; see [`Self::transition_for`].
    #[must_use]
    pub const fn supersedes(&self) -> Option<CurriculumVersionId> {
        self.supersedes
    }

    /// Every recorded transitional arrangement.
    #[must_use]
    pub fn transitions(&self) -> &[TransitionArrangement] {
        &self.transitions
    }

    /// Section 8.2's `effectiveFrom`/`effectiveTo`.
    #[must_use]
    pub const fn valid_time(&self) -> ValidInterval {
        self.valid_time
    }

    /// What a recorded arrangement says about one cohort.
    ///
    /// Reads [`Self::transitions`] and nothing else. A version that supersedes
    /// another and records no arrangement answers
    /// [`CohortTransition::Unknown`] for every cohort.
    #[must_use]
    pub fn transition_for(&self, cohort: &AdmissionCohort) -> CohortTransition {
        self.transitions
            .iter()
            .find(|arrangement| &arrangement.cohort == cohort)
            .map_or(
                CohortTransition::Unknown,
                TransitionArrangement::disposition,
            )
    }
}

/// The only route to a [`CurriculumVersion`].
#[derive(Debug, Clone)]
pub struct CurriculumVersionDraft {
    id: CurriculumVersionId,
    institution_path: Vec<String>,
    admission_year_range: (AdmissionCohort, AdmissionCohort),
    status: PublicationStatus,
    source_snapshot: Option<ContentDigest>,
    supersedes: Option<CurriculumVersionId>,
    transitions: Vec<TransitionArrangement>,
    valid_time: ValidInterval,
}

impl CurriculumVersionDraft {
    /// Starts a draft. `status` begins at [`PublicationStatus::Unknown`].
    #[must_use]
    pub const fn new(
        id: CurriculumVersionId,
        admission_year_range: (AdmissionCohort, AdmissionCohort),
        valid_time: ValidInterval,
    ) -> Self {
        Self {
            id,
            institution_path: Vec::new(),
            admission_year_range,
            status: PublicationStatus::Unknown,
            source_snapshot: None,
            supersedes: None,
            transitions: Vec::new(),
            valid_time,
        }
    }

    /// Appends one institution-path segment.
    #[must_use]
    pub fn institution_segment(mut self, segment: &str) -> Self {
        self.institution_path.push(segment.to_owned());
        self
    }

    /// Records a confirmed publication status.
    #[must_use]
    pub const fn status(mut self, status: PublicationStatus) -> Self {
        self.status = status;
        self
    }

    /// Records the official source snapshot digest.
    #[must_use]
    pub const fn source_snapshot(mut self, digest: ContentDigest) -> Self {
        self.source_snapshot = Some(digest);
        self
    }

    /// Records which version this one supersedes.
    #[must_use]
    pub const fn supersedes(mut self, earlier: CurriculumVersionId) -> Self {
        self.supersedes = Some(earlier);
        self
    }

    /// Appends one transitional arrangement.
    #[must_use]
    pub fn transition(mut self, arrangement: TransitionArrangement) -> Self {
        self.transitions.push(arrangement);
        self
    }

    /// Builds the version.
    pub fn build(self) -> Result<CurriculumVersion, CurriculumError> {
        if self.institution_path.is_empty() {
            return Err(CurriculumError::Missing {
                aggregate: "curriculum version",
                field: "institution path",
            });
        }
        if self.supersedes == Some(self.id) {
            return Err(CurriculumError::Reflexive {
                relation: "supersession",
            });
        }
        Ok(CurriculumVersion {
            id: self.id,
            institution_path: self.institution_path,
            admission_year_range: self.admission_year_range,
            status: self.status,
            source_snapshot: self.source_snapshot,
            supersedes: self.supersedes,
            transitions: self.transitions,
            valid_time: self.valid_time,
        })
    }
}
