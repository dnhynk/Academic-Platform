//! What section 29.5's record reads out of a review, and the scale it is not.
//!
//! The `dimensions:` block of section 29.5's `ReviewRecord` names its keys and
//! this module names the same ones. `the_review_dimensions_are_section_29_5s_own`
//! reads the block out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`
//! and compares the key list against [`ReviewDimension::ALL`] in both
//! directions, so a dimension added here without the specification, or dropped
//! here while the specification still names it, fails. It enumerates them; it
//! asserts no count of them.
//!
//! # A reading is a band, not a number a course carries
//!
//! Section 29.5 ends: *"난이도 4.2"를 객관적 과목 속성으로 쓰지 않는다.*
//! [`DimensionBand`] is five ordered bands and no arithmetic: it derives no
//! `Add`, no `Sum`, and there is no function anywhere in this crate from a set
//! of bands to a band, because a mean of five bands is the scalar that sentence
//! refuses. What an aggregate reports is the distribution — how many readings
//! fell in each band — and [`crate::aggregate::BandDistribution`] is that.
//!
//! A band still has an ordinal spelling, because "harder than" is a real
//! comparison a reader makes. What it does not have is a spacing: nothing here
//! says the distance from `VeryLow` to `Low` equals the distance from `Low` to
//! `Moderate`, and without a spacing there is no mean to take.

use crate::error::ReviewError;

/// The nine things section 29.5's record reads out of a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewDimension {
    /// `difficulty`.
    Difficulty,
    /// `workload`.
    Workload,
    /// `assessmentStyle`.
    AssessmentStyle,
    /// `projectWeight`.
    ProjectWeight,
    /// `theoryImplementationBalance`.
    TheoryImplementationBalance,
    /// `mathematicalRigor`.
    MathematicalRigor,
    /// `materialQuality`.
    MaterialQuality,
    /// `explanationStyle`.
    ExplanationStyle,
    /// `teamProject`.
    TeamProject,
}

impl ReviewDimension {
    /// Section 29.5's order, which is the order its record lists the keys in.
    pub const ALL: [Self; 9] = [
        Self::Difficulty,
        Self::Workload,
        Self::AssessmentStyle,
        Self::ProjectWeight,
        Self::TheoryImplementationBalance,
        Self::MathematicalRigor,
        Self::MaterialQuality,
        Self::ExplanationStyle,
        Self::TeamProject,
    ];

    /// The key section 29.5's record spells this dimension with.
    #[must_use]
    pub const fn spec_key(self) -> &'static str {
        match self {
            Self::Difficulty => "difficulty",
            Self::Workload => "workload",
            Self::AssessmentStyle => "assessmentStyle",
            Self::ProjectWeight => "projectWeight",
            Self::TheoryImplementationBalance => "theoryImplementationBalance",
            Self::MathematicalRigor => "mathematicalRigor",
            Self::MaterialQuality => "materialQuality",
            Self::ExplanationStyle => "explanationStyle",
            Self::TeamProject => "teamProject",
        }
    }

    /// This dimension's position in [`Self::ALL`].
    ///
    /// A total `match` rather than a search, so
    /// [`ReviewExtraction::reading`] is a direct index with no arm that stands
    /// in for "not found". `the_dimension_index_is_its_position_in_all` walks
    /// [`Self::ALL`] and requires the two to agree.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Difficulty => 0,
            Self::Workload => 1,
            Self::AssessmentStyle => 2,
            Self::ProjectWeight => 3,
            Self::TheoryImplementationBalance => 4,
            Self::MathematicalRigor => 5,
            Self::MaterialQuality => 6,
            Self::ExplanationStyle => 7,
            Self::TeamProject => 8,
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Difficulty => "DIFFICULTY",
            Self::Workload => "WORKLOAD",
            Self::AssessmentStyle => "ASSESSMENT_STYLE",
            Self::ProjectWeight => "PROJECT_WEIGHT",
            Self::TheoryImplementationBalance => "THEORY_IMPLEMENTATION_BALANCE",
            Self::MathematicalRigor => "MATHEMATICAL_RIGOR",
            Self::MaterialQuality => "MATERIAL_QUALITY",
            Self::ExplanationStyle => "EXPLANATION_STYLE",
            Self::TeamProject => "TEAM_PROJECT",
        }
    }
}

/// Where one reader put one dimension.
///
/// Ordered, and that is all it is. There is no numeric value behind a band and
/// no conversion to one: `scalar_is_not_a_course_property` compares this type's
/// whole `impl` inventory and its derive list, so an `Into<u8>` added later
/// fails as an extra entry rather than quietly making "difficulty 4.2"
/// expressible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionBand {
    /// The lowest band a reader used.
    VeryLow,
    /// Below the middle.
    Low,
    /// The middle band.
    Moderate,
    /// Above the middle.
    High,
    /// The highest band a reader used.
    VeryHigh,
}

impl DimensionBand {
    /// Ascending order.
    pub const ALL: [Self; 5] = [
        Self::VeryLow,
        Self::Low,
        Self::Moderate,
        Self::High,
        Self::VeryHigh,
    ];

    /// This band's position in [`Self::ALL`].
    ///
    /// A total `match`, so a distribution indexes by a number this enum states
    /// rather than by a discriminant a reorder would move.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::VeryLow => 0,
            Self::Low => 1,
            Self::Moderate => 2,
            Self::High => 3,
            Self::VeryHigh => 4,
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VeryLow => "VERY_LOW",
            Self::Low => "LOW",
            Self::Moderate => "MODERATE",
            Self::High => "HIGH",
            Self::VeryHigh => "VERY_HIGH",
        }
    }
}

/// One dimension, as one review read it, with where it was read from.
///
/// The span index points into the review's own [`crate::text::ProvenanceSpan`]
/// list, so a reading always says which part of the text it came from and the
/// text itself stays where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimensionReading {
    dimension: ReviewDimension,
    band: DimensionBand,
    span_index: usize,
}

impl DimensionReading {
    /// Records one reading against one span of the review.
    #[must_use]
    pub const fn new(dimension: ReviewDimension, band: DimensionBand, span_index: usize) -> Self {
        Self {
            dimension,
            band,
            span_index,
        }
    }

    /// Which dimension.
    #[must_use]
    pub const fn dimension(self) -> ReviewDimension {
        self.dimension
    }

    /// Which band.
    #[must_use]
    pub const fn band(self) -> DimensionBand {
        self.band
    }

    /// Which of the review's provenance spans this was read from.
    #[must_use]
    pub const fn span_index(self) -> usize {
        self.span_index
    }
}

/// A complete reading of one review: every dimension, once.
///
/// Private field and one constructor. The constructor names the first dimension
/// with no reading, so `the_extraction_reads_every_dimension` is per-dimension
/// evidence: it drops each of [`ReviewDimension::ALL`] in turn and requires the
/// exact error for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewExtraction {
    readings: Vec<DimensionReading>,
}

impl ReviewExtraction {
    /// Takes one reading per dimension, in section 29.5's order.
    ///
    /// # Errors
    ///
    /// [`ReviewError::DimensionRepeated`] when one dimension is read twice, and
    /// [`ReviewError::DimensionMissing`] naming the first of
    /// [`ReviewDimension::ALL`] nothing read.
    pub fn read(readings: &[DimensionReading]) -> Result<Self, ReviewError> {
        let mut ordered = Vec::with_capacity(ReviewDimension::ALL.len());
        for dimension in ReviewDimension::ALL {
            let mut found = readings
                .iter()
                .filter(|reading| reading.dimension() == dimension);
            let Some(reading) = found.next() else {
                return Err(ReviewError::DimensionMissing(dimension));
            };
            if found.next().is_some() {
                return Err(ReviewError::DimensionRepeated(dimension));
            }
            ordered.push(*reading);
        }
        Ok(Self { readings: ordered })
    }

    /// Every reading, in [`ReviewDimension::ALL`] order.
    #[must_use]
    pub fn readings(&self) -> &[DimensionReading] {
        &self.readings
    }

    /// How one dimension was read.
    ///
    /// A direct index. [`Self::read`] is the only producer and it stores one
    /// reading per dimension in [`ReviewDimension::ALL`] order, so
    /// [`ReviewDimension::index`] is the position and there is no arm here that
    /// means "not found".
    #[must_use]
    pub fn reading(&self, dimension: ReviewDimension) -> DimensionReading {
        self.readings[dimension.index()]
    }

    /// The band one dimension was read at.
    #[must_use]
    pub fn band(&self, dimension: ReviewDimension) -> DimensionBand {
        self.reading(dimension).band()
    }
}
