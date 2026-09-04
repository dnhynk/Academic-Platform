//! What a review is attached to, and the one thing it can never be attached to.
//!
//! Section 29.5's first sentence: *Review는 기본적으로 `CourseOffering +
//! Instructor + Term + Source`에 연결하고 Course 전체로 승격할 때 명시적
//! aggregation을 사용한다.* [`ScopeDimension`] is that list and
//! `review_default_scope_is_offering_instructor_term_source` reads the sentence
//! out of the specification and walks it forwards, so the four cannot drift
//! from it.
//!
//! # There is no course here
//!
//! [`ReviewScope`] has no `CourseId` field, no constructor that takes one, and
//! no accessor that returns one. Section 34's own failure row is *Course와
//! Offering 혼동 — catalog row에 교수·학기 속성을 덮어씀*, and the prevention
//! column is *별도 aggregate*. A scope that could name a course would be the
//! overwrite that row describes, so the course-level value is
//! [`crate::aggregate::CourseAggregate`] and it is reached from an
//! [`crate::aggregate::AggregationClaim`] rather than from a review.
//!
//! # Three of the four are optional and the fourth is not
//!
//! The section 29.5 record writes `offering`, `instructor` and `term` as
//! `... | null`: a review found on a page that names no instructor is still a
//! review of that offering. The source is not optional and has no null
//! spelling, because a review with no source has no provenance and section 29.5
//! keeps the raw artifact *for* provenance.

use academic_curriculum::{InstructorName, TermCode};
use academic_domain::OfferingId;
use academic_ingestion::ConnectorId;

/// The four things section 29.5 attaches a review to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeDimension {
    /// The `CourseOffering` the review is about.
    Offering,
    /// The instructor who taught it.
    Instructor,
    /// The term it ran in.
    Term,
    /// Where the review text came from.
    Source,
}

impl ScopeDimension {
    /// Section 29.5's order, which is the order its sentence lists them in.
    pub const ALL: [Self; 4] = [Self::Offering, Self::Instructor, Self::Term, Self::Source];

    /// The name section 29.5's sentence spells this dimension with.
    ///
    /// `review_default_scope_is_offering_instructor_term_source` requires these
    /// to appear in the specification's sentence, in this order, and requires
    /// the sentence to hold nothing else.
    #[must_use]
    pub const fn spec_name(self) -> &'static str {
        match self {
            Self::Offering => "CourseOffering",
            Self::Instructor => "Instructor",
            Self::Term => "Term",
            Self::Source => "Source",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offering => "OFFERING",
            Self::Instructor => "INSTRUCTOR",
            Self::Term => "TERM",
            Self::Source => "SOURCE",
        }
    }

    /// Whether section 29.5 writes this dimension as `... | null`.
    ///
    /// The source is the one that is not, and
    /// `a_review_scope_has_no_course` observes there is no way to build a scope
    /// without it.
    #[must_use]
    pub const fn is_nullable(self) -> bool {
        !matches!(self, Self::Source)
    }
}

/// What one review is attached to.
///
/// Private fields, one constructor, and no setter. Two scopes that differ in
/// any dimension are two scopes: [`Self::same_scope_as`] compares all four, and
/// nothing here merges them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewScope {
    offering: Option<OfferingId>,
    instructor: Option<InstructorName>,
    term: Option<TermCode>,
    source: ConnectorId,
}

impl ReviewScope {
    /// Attaches a review to a source, and to whichever of the other three are
    /// known.
    ///
    /// The source is a positional argument rather than an `Option` because it
    /// is the dimension section 29.5 does not write as nullable.
    #[must_use]
    pub const fn new(
        source: ConnectorId,
        offering: Option<OfferingId>,
        instructor: Option<InstructorName>,
        term: Option<TermCode>,
    ) -> Self {
        Self {
            offering,
            instructor,
            term,
            source,
        }
    }

    /// Which offering, when the review names one.
    #[must_use]
    pub const fn offering(&self) -> Option<OfferingId> {
        self.offering
    }

    /// Which instructor, when the review names one.
    #[must_use]
    pub const fn instructor(&self) -> Option<&InstructorName> {
        self.instructor.as_ref()
    }

    /// Which term, when the review names one.
    #[must_use]
    pub const fn term(&self) -> Option<&TermCode> {
        self.term.as_ref()
    }

    /// Where the text came from. Always present.
    #[must_use]
    pub const fn source(&self) -> &ConnectorId {
        &self.source
    }

    /// Whether a dimension carries a value.
    #[must_use]
    pub fn carries(&self, dimension: ScopeDimension) -> bool {
        match dimension {
            ScopeDimension::Offering => self.offering.is_some(),
            ScopeDimension::Instructor => self.instructor.is_some(),
            ScopeDimension::Term => self.term.is_some(),
            ScopeDimension::Source => true,
        }
    }

    /// Whether two reviews are about the same thing.
    ///
    /// All four dimensions, with no fallback to a subset. Section 34's *Course와
    /// Offering 혼동* row is what a partial comparison produces: two offerings
    /// of one course, taught by different people, read as one.
    #[must_use]
    pub fn same_scope_as(&self, other: &Self) -> bool {
        self == other
    }
}
