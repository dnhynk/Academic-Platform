//! The six things every aggregate discloses, and why none of them is optional.
//!
//! Section 29.5's last paragraph: *강의평 aggregate는 표본 수, 최근성, 교수/학기
//! mix, 응답자 self-selection, 극단 경험 편향, 중복 가능성을 표시한다.*
//! [`BiasDimension`] is that list.
//! `the_bias_dimensions_are_section_29_5s_own` reads the sentence out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, maps each Korean
//! phrase to a variant, and walks the sentence forwards, so the six cannot
//! drift from it. It enumerates them; it asserts no count of them.
//!
//! Section 34's *강의평 편향* row is the same list from the failure side --
//! *self-selection, 소수 표본, 오래된 학기, 교수 혼합, 중복* -- with the
//! detection column *sample size/time/instructor distribution, duplicate
//! similarity* and the uncertainty column *표본·범위·편향 경고, 단일 score
//! 비기본*. That is why a [`BiasDisclosure`] is a required argument of every
//! aggregate constructor rather than a field a caller may leave empty: a warning
//! nobody has to supply is a warning that is absent exactly when the sample is
//! worst.
//!
//! # The disclosure is built by naming every dimension
//!
//! [`BiasDisclosureDraft`] is the only route to a [`BiasDisclosure`], and
//! [`BiasDisclosureDraft::build`] names the first dimension nothing was
//! recorded for. `aggregate_discloses_all_six_bias_dimensions` iterates
//! [`BiasDimension::ALL`], rebuilds a complete draft with exactly one dimension
//! dropped, and requires the exact
//! [`crate::error::ReviewError::BiasDimensionMissing`] for it. The evidence is
//! per-dimension, not a count.

use crate::error::ReviewError;

/// The six disclosures section 29.5 requires of a review aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BiasDimension {
    /// 표본 수 -- how many reviews the aggregate is over.
    SampleCount,
    /// 최근성 -- how old the newest and oldest of them are.
    Recency,
    /// 교수/학기 mix -- how many instructors and terms are mixed together.
    InstructorTermMix,
    /// 응답자 self-selection -- who chose to write at all.
    SelfSelection,
    /// 극단 경험 편향 -- the pull of the best and worst experiences.
    ExtremeExperience,
    /// 중복 가능성 -- how much of the sample may be one text twice.
    Duplication,
}

impl BiasDimension {
    /// Section 29.5's order, which is the order its sentence lists them in.
    pub const ALL: [Self; 6] = [
        Self::SampleCount,
        Self::Recency,
        Self::InstructorTermMix,
        Self::SelfSelection,
        Self::ExtremeExperience,
        Self::Duplication,
    ];

    /// The phrase section 29.5's sentence names this dimension with.
    ///
    /// `the_bias_dimensions_are_section_29_5s_own` requires each of these to
    /// appear in the specification's sentence, in this order, and requires the
    /// sentence's comma-separated items to be exactly these six.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::SampleCount => "표본 수",
            Self::Recency => "최근성",
            Self::InstructorTermMix => "교수/학기 mix",
            Self::SelfSelection => "응답자 self-selection",
            Self::ExtremeExperience => "극단 경험 편향",
            Self::Duplication => "중복 가능성",
        }
    }

    /// This dimension's position in [`Self::ALL`].
    ///
    /// A total `match`, for the reason
    /// [`crate::dimension::ReviewDimension::index`] gives.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::SampleCount => 0,
            Self::Recency => 1,
            Self::InstructorTermMix => 2,
            Self::SelfSelection => 3,
            Self::ExtremeExperience => 4,
            Self::Duplication => 5,
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SampleCount => "SAMPLE_COUNT",
            Self::Recency => "RECENCY",
            Self::InstructorTermMix => "INSTRUCTOR_TERM_MIX",
            Self::SelfSelection => "SELF_SELECTION",
            Self::ExtremeExperience => "EXTREME_EXPERIENCE",
            Self::Duplication => "DUPLICATION",
        }
    }
}

/// How strongly one dimension warns.
///
/// Three ordered levels and no numeric conversion, for the reason
/// [`crate::dimension::DimensionBand`] gives: a level a caller can average is
/// a level a caller can hide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BiasStrength {
    /// The measurement is present and does not warn.
    Low,
    /// The measurement warns.
    Elevated,
    /// The measurement warns and the aggregate should not be read alone.
    Severe,
}

impl BiasStrength {
    /// Ascending order.
    pub const ALL: [Self; 3] = [Self::Low, Self::Elevated, Self::Severe];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Elevated => "ELEVATED",
            Self::Severe => "SEVERE",
        }
    }
}

/// One dimension's disclosure: the measurement, and how strongly it warns.
///
/// `measured` is what the aggregate counted -- a sample size, a term span, a
/// number of distinct instructors, a duplicate-pair count. It is a count and
/// never a review's words: `every_field_of_every_type_is_classified` classifies
/// it, so a text field added here fails as an unclassified field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiasFinding {
    dimension: BiasDimension,
    measured: u32,
    strength: BiasStrength,
}

impl BiasFinding {
    /// Records one dimension's measurement and warning level.
    #[must_use]
    pub const fn new(dimension: BiasDimension, measured: u32, strength: BiasStrength) -> Self {
        Self {
            dimension,
            measured,
            strength,
        }
    }

    /// Which dimension.
    #[must_use]
    pub const fn dimension(self) -> BiasDimension {
        self.dimension
    }

    /// What was counted.
    #[must_use]
    pub const fn measured(self) -> u32 {
        self.measured
    }

    /// How strongly it warns.
    #[must_use]
    pub const fn strength(self) -> BiasStrength {
        self.strength
    }
}

/// A draft disclosure, before every dimension has been named.
///
/// The only route to a [`BiasDisclosure`]. There is no `Default`: an empty
/// draft is spelled [`BiasDisclosureDraft::new`] and it builds nothing.
#[derive(Debug, Clone, Default)]
pub struct BiasDisclosureDraft {
    findings: Vec<BiasFinding>,
}

impl BiasDisclosureDraft {
    /// An empty draft.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            findings: Vec::new(),
        }
    }

    /// Records one dimension.
    #[must_use]
    pub fn disclosing(mut self, finding: BiasFinding) -> Self {
        self.findings.push(finding);
        self
    }

    /// Builds the disclosure, or names the first dimension nothing disclosed.
    ///
    /// # Errors
    ///
    /// [`ReviewError::BiasDimensionRepeated`] when a dimension is disclosed
    /// twice, and [`ReviewError::BiasDimensionMissing`] naming the first of
    /// [`BiasDimension::ALL`] nothing disclosed.
    pub fn build(self) -> Result<BiasDisclosure, ReviewError> {
        let mut ordered = Vec::with_capacity(BiasDimension::ALL.len());
        for dimension in BiasDimension::ALL {
            let mut found = self
                .findings
                .iter()
                .filter(|finding| finding.dimension() == dimension);
            let Some(finding) = found.next() else {
                return Err(ReviewError::BiasDimensionMissing(dimension));
            };
            if found.next().is_some() {
                return Err(ReviewError::BiasDimensionRepeated(dimension));
            }
            ordered.push(*finding);
        }
        Ok(BiasDisclosure { findings: ordered })
    }
}

/// Every one of section 29.5's six disclosures, present.
///
/// Private field, no `Default`, and [`BiasDisclosureDraft::build`] is the only
/// producer. A disclosure that exists is a disclosure that names all six, which
/// is why [`crate::aggregate::OfferingAggregate`] and
/// [`crate::aggregate::CourseAggregate`] take one by value rather than
/// assembling one: there is no partial value to hand them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiasDisclosure {
    findings: Vec<BiasFinding>,
}

impl BiasDisclosure {
    /// Every finding, in [`BiasDimension::ALL`] order.
    #[must_use]
    pub fn findings(&self) -> &[BiasFinding] {
        &self.findings
    }

    /// One dimension's finding.
    ///
    /// A direct index: the producer stores one finding per dimension in
    /// [`BiasDimension::ALL`] order, so [`BiasDimension::index`] is the
    /// position and there is no arm here that means "not disclosed".
    #[must_use]
    pub fn finding(&self, dimension: BiasDimension) -> BiasFinding {
        self.findings[dimension.index()]
    }

    /// The dimensions this disclosure names, in order.
    ///
    /// Always the whole of [`BiasDimension::ALL`]. It is derived from the
    /// findings rather than returned as a constant, so a disclosure that
    /// somehow held fewer would report fewer rather than claim six.
    #[must_use]
    pub fn disclosed(&self) -> Vec<BiasDimension> {
        self.findings.iter().map(|f| f.dimension()).collect()
    }
}
