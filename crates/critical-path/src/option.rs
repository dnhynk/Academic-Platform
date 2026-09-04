//! Section 16.2's last sentence: `Course 수강은 concept 획득 그 자체가 아니라
//! 여러 exposure/practice 기회를 묶은 acquisition option이다`.
//!
//! ## What the type refuses
//!
//! `course_is_an_acquisition_option` is not a check somebody runs. An
//! [`AcquisitionOption`] has **no function returning a mastery level, a
//! knowledge state, or a satisfied concept**. What it hands out is
//! [`Opportunity`] values, each of which names a concept and an occasion, and
//! an occasion is not evidence: `P2-N2` decides what evidence licenses, and
//! nothing in this crate can produce an `EligibleEvidence` or a
//! `KnowledgeStateAssertion`.
//!
//! So a plan that takes a course still reports the concept as one the goal
//! needs. The course changes the plan's `evidence_opportunity` benefit and its
//! `calendar_delay` cost; it does not change what the user knows, and there is
//! no code path by which it could.
//!
//! ## A course is one option among several
//!
//! [`AcquisitionOption`] has three variants and a course is one of them. That
//! is the other half of the sentence: taking the course is a *way* of getting
//! exposure and practice, not the definition of getting it. Section 36.7 is the
//! design document's own worked case -- `isolation` is supported by the current
//! offering, `idempotent API design` is not, and external reading plus a project
//! experiment is the better option for it -- so the two live in one enumeration
//! and a plan chooses between them by vector rather than by kind.

use academic_curriculum::{Credits, OfferingStatus};
use academic_domain::{EntityId, EvidenceId, OfferingId};
use serde::{Deserialize, Serialize};

use crate::CriticalPathError;

/// What kind of occasion an option supplies.
///
/// Section 16.2 names `exposure` and `practice`; section 22's projected lane
/// adds an assessment occasion, and `P2-N8` owns that lane. All three are
/// occasions and none of them is evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpportunityKind {
    /// `exposure`: the concept is presented.
    Exposure,
    /// `practice`: the concept is used on a problem.
    Practice,
    /// An occasion on which performance would be observed.
    Assessment,
}

impl OpportunityKind {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exposure => "EXPOSURE",
            Self::Practice => "PRACTICE",
            Self::Assessment => "ASSESSMENT",
        }
    }
}

/// One occasion an option supplies toward one concept.
///
/// Carries no mastery, no confidence and no evidence identity of its own. The
/// `source` is what the occasion *is* -- a lecture node, a problem set, a
/// repository file -- and it is a locator the user can open, not a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opportunity {
    concept: EntityId,
    kind: OpportunityKind,
    source: EvidenceId,
}

impl Opportunity {
    /// Records one occasion.
    #[must_use]
    pub const fn of(concept: EntityId, kind: OpportunityKind, source: EvidenceId) -> Self {
        Self {
            concept,
            kind,
            source,
        }
    }

    /// Which concept the occasion is about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// What kind of occasion.
    #[must_use]
    pub const fn kind(&self) -> OpportunityKind {
        self.kind
    }

    /// What the user would open.
    #[must_use]
    pub const fn source(&self) -> EvidenceId {
        self.source
    }
}

/// A way of getting the occasions a concept needs.
///
/// Three variants, one of which is a course. See the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquisitionOption {
    /// Section 16.2's `Course 수강`: an offering, its standing, its credits and
    /// the occasions it bundles.
    Course {
        /// Which offering.
        offering: OfferingId,
        /// Section 8.3's standing for it. Read from `P2-U1`, never decided here.
        status: OfferingStatus,
        /// The credits it costs against a term limit.
        credits: Credits,
        /// The occasions it bundles. Never empty, and never fewer than one
        /// exposure and one practice.
        opportunities: Vec<Opportunity>,
    },
    /// Section 36.7's `external reading`.
    SelfStudy {
        /// The occasions the material supplies. Never empty.
        opportunities: Vec<Opportunity>,
    },
    /// Section 36.7's `project experiment`, and section 36.4's
    /// `page-layout experiment`.
    ProjectWork {
        /// Which project the work happens in.
        project: EntityId,
        /// The occasions it supplies. Never empty.
        opportunities: Vec<Opportunity>,
    },
}

impl AcquisitionOption {
    /// Declares a course option.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::OptionSuppliesNoOpportunity`] for an empty list,
    /// and [`CriticalPathError::CourseIsNotABundle`] for a course that supplies
    /// no exposure or no practice. Section 16.2 calls a course `여러
    /// exposure/practice 기회를 묶은` option; one that bundles neither is being
    /// modelled as an acquisition, which is the sentence's own refusal.
    pub fn course(
        offering: OfferingId,
        status: OfferingStatus,
        credits: Credits,
        opportunities: Vec<Opportunity>,
    ) -> Result<Self, CriticalPathError> {
        if opportunities.is_empty() {
            return Err(CriticalPathError::OptionSuppliesNoOpportunity);
        }
        let bundles = |kind: OpportunityKind| {
            opportunities
                .iter()
                .any(|opportunity| opportunity.kind() == kind)
        };
        if !bundles(OpportunityKind::Exposure) || !bundles(OpportunityKind::Practice) {
            return Err(CriticalPathError::CourseIsNotABundle);
        }
        Ok(Self::Course {
            offering,
            status,
            credits,
            opportunities,
        })
    }

    /// Declares a self-study option.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::OptionSuppliesNoOpportunity`] for an empty list.
    pub fn self_study(opportunities: Vec<Opportunity>) -> Result<Self, CriticalPathError> {
        if opportunities.is_empty() {
            return Err(CriticalPathError::OptionSuppliesNoOpportunity);
        }
        Ok(Self::SelfStudy { opportunities })
    }

    /// Declares a project-work option.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::OptionSuppliesNoOpportunity`] for an empty list.
    pub fn project_work(
        project: EntityId,
        opportunities: Vec<Opportunity>,
    ) -> Result<Self, CriticalPathError> {
        if opportunities.is_empty() {
            return Err(CriticalPathError::OptionSuppliesNoOpportunity);
        }
        Ok(Self::ProjectWork {
            project,
            opportunities,
        })
    }

    /// The occasions this option supplies. Never empty.
    #[must_use]
    pub fn supplies(&self) -> &[Opportunity] {
        match self {
            Self::Course { opportunities, .. }
            | Self::SelfStudy { opportunities }
            | Self::ProjectWork { opportunities, .. } => opportunities,
        }
    }

    /// The occasions this option supplies toward one concept.
    #[must_use]
    pub fn supplies_toward(&self, concept: EntityId) -> Vec<&Opportunity> {
        self.supplies()
            .iter()
            .filter(|opportunity| opportunity.concept() == concept)
            .collect()
    }

    /// The offering, when this option is a course.
    #[must_use]
    pub const fn offering(&self) -> Option<OfferingId> {
        match self {
            Self::Course { offering, .. } => Some(*offering),
            Self::SelfStudy { .. } | Self::ProjectWork { .. } => None,
        }
    }

    /// The credits this option costs against a term limit. Zero outside a
    /// course, because nothing else is registered for.
    #[must_use]
    pub const fn credits(&self) -> u8 {
        match self {
            Self::Course { credits, .. } => credits.value(),
            Self::SelfStudy { .. } | Self::ProjectWork { .. } => 0,
        }
    }

    /// Section 8.3's standing, when this option is a course.
    #[must_use]
    pub const fn offering_status(&self) -> Option<OfferingStatus> {
        match self {
            Self::Course { status, .. } => Some(*status),
            Self::SelfStudy { .. } | Self::ProjectWork { .. } => None,
        }
    }

    /// Stable spelling of which option this is.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Course { .. } => "COURSE",
            Self::SelfStudy { .. } => "SELF_STUDY",
            Self::ProjectWork { .. } => "PROJECT_WORK",
        }
    }
}
