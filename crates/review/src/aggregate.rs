//! What a set of reviews becomes, and the one way it reaches a course.
//!
//! Section 29.5: *Review는 기본적으로 `CourseOffering + Instructor + Term +
//! Source`에 연결하고 **Course 전체로 승격할 때 명시적 aggregation을
//! 사용한다**.*
//!
//! # Two producers, and no path from one to the other
//!
//! [`OfferingAggregate::over`] takes reviews that share a scope and produces an
//! offering-level value. [`CourseAggregate::promote`] takes an
//! [`AggregationClaim`] and produces a course-level one. Neither takes the
//! other's argument, and there is no function in this crate that converts
//! between them:
//!
//! * [`AggregationClaim`] has private fields, no `Default`, and one
//!   constructor, [`AggregationClaim::asserting`], whose first argument is an
//!   [`AggregationMethod`] -- an enum with no `Unknown` arm and no arm a value
//!   gets by not deciding.
//! * [`CourseAggregate::promote`] is the only producer of a
//!   [`CourseAggregate`] and its only route to one is that claim, taken **by
//!   value**. [`AggregationClaim`] derives no `Clone` and no `Copy`, so a claim
//!   is spent once.
//! * There is no `From<OfferingAggregate> for CourseAggregate`, no
//!   `TryFrom`, and no constructor on [`CourseAggregate`] that takes offering
//!   aggregates alone. `tests/compile_fail/` observes both halves: assembling
//!   the claim from outside, and handing `promote` a bare list of offering
//!   aggregates.
//!
//! `course_promotion_requires_explicit_aggregation` drives the behavioural
//! half **at each producer separately**, because a guard written twice and
//! driven once is a guard that can be relaxed at the undriven site and still
//! pass -- `T186` measured exactly that in `P2-U5`'s crate.
//!
//! # No scalar reaches a course
//!
//! Section 29.5's last sentence refuses *"난이도 4.2"* as an objective course
//! property. Three things carry that here.
//!
//! * A course-level reading is a [`BandDistribution`] -- the count of reviews
//!   in each of [`crate::dimension::DimensionBand::ALL`] -- and never a value.
//!   There is no mean, no median and no representative band, because a
//!   representative band is the scalar under another name.
//! * A [`BandDistribution`] cannot be obtained without the aggregate that holds
//!   it, and an aggregate cannot be obtained without a
//!   [`crate::bias::BiasDisclosure`]. So there is no value in this crate that
//!   is a course reading without its six warnings attached.
//! * `academic-curriculum`'s `Course` has three fields and this crate is not
//!   one of them: it holds an identifier, a code and a canonical identity, and
//!   `scalar_is_not_a_course_property` reads that struct's whole field list out
//!   of its source and requires every dimension name to be absent from it, in
//!   both directions. A [`CourseAggregate`] names a `CourseId`; a `Course` does
//!   not name a [`CourseAggregate`], and `academic-curriculum` has no edge of
//!   any kind to this crate.

use academic_curriculum::{InstructorName, TermCode};
use academic_domain::{CourseId, OfferingId};

use crate::{
    bias::BiasDisclosure,
    dimension::{DimensionBand, ReviewDimension},
    error::ReviewError,
    record::ReviewRecord,
    scope::{ReviewScope, ScopeDimension},
};

/// How many reviews fell in each band, for one dimension.
///
/// The counts are in [`DimensionBand::ALL`] order. There is no accessor that
/// reduces them: no mean, no median, no mode, no "representative" band. A
/// reader sees the shape of the sample or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandDistribution {
    dimension: ReviewDimension,
    counts: [u32; 5],
}

impl BandDistribution {
    /// Which dimension.
    #[must_use]
    pub const fn dimension(self) -> ReviewDimension {
        self.dimension
    }

    /// How many reviews fell in one band.
    #[must_use]
    pub fn count(self, band: DimensionBand) -> u32 {
        self.counts[band.index()]
    }

    /// The counts, in [`DimensionBand::ALL`] order.
    #[must_use]
    pub const fn counts(self) -> [u32; 5] {
        self.counts
    }

    /// How many readings the distribution is over.
    #[must_use]
    pub fn total(self) -> u32 {
        self.counts.iter().copied().fold(0, u32::saturating_add)
    }
}

/// One offering's reviews, aggregated.
///
/// Every review in it shares one [`ReviewScope`]; [`Self::over`] refuses a set
/// that does not and names the dimension that differed. Section 34's *Course와
/// Offering 혼동* row is the failure that refusal is about: two offerings of one
/// course, taught by different people, read as one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferingAggregate {
    scope: ReviewScope,
    sample_size: u32,
    distributions: Vec<BandDistribution>,
    disclosure: BiasDisclosure,
}

impl OfferingAggregate {
    /// Aggregates reviews that share one scope.
    ///
    /// # Errors
    ///
    /// [`ReviewError::NoReviews`] for an empty set, and
    /// [`ReviewError::ScopeMixed`] naming the first dimension on which two of
    /// them disagree.
    pub fn over(records: &[ReviewRecord], disclosure: BiasDisclosure) -> Result<Self, ReviewError> {
        let Some(first) = records.first() else {
            return Err(ReviewError::NoReviews);
        };
        let scope = first.scope();
        for record in records {
            if let Some(dimension) = differing_dimension(scope, record.scope()) {
                return Err(ReviewError::ScopeMixed(dimension));
            }
        }
        let mut distributions = Vec::with_capacity(ReviewDimension::ALL.len());
        for dimension in ReviewDimension::ALL {
            let mut counts = [0_u32; 5];
            for record in records {
                let band = record.band(dimension);
                counts[band.index()] = counts[band.index()].saturating_add(1);
            }
            distributions.push(BandDistribution { dimension, counts });
        }
        Ok(Self {
            scope: scope.clone(),
            // Saturating for the reason `crate::duplicate` gives: a sample
            // larger than a `u32` is not one this crate will see, and the
            // maximum is the only honest reading of one.
            sample_size: u32::try_from(records.len()).unwrap_or(u32::MAX),
            distributions,
            disclosure,
        })
    }

    /// What this aggregate is scoped to.
    #[must_use]
    pub const fn scope(&self) -> &ReviewScope {
        &self.scope
    }

    /// How many reviews it is over.
    #[must_use]
    pub const fn sample_size(&self) -> u32 {
        self.sample_size
    }

    /// One dimension's distribution.
    #[must_use]
    pub fn distribution(&self, dimension: ReviewDimension) -> BandDistribution {
        self.distributions[dimension.index()]
    }

    /// Every distribution, in [`ReviewDimension::ALL`] order.
    #[must_use]
    pub fn distributions(&self) -> &[BandDistribution] {
        &self.distributions
    }

    /// The six disclosures. Always present; see [`crate::bias`].
    #[must_use]
    pub const fn disclosure(&self) -> &BiasDisclosure {
        &self.disclosure
    }

    /// Which offering, when the scope names one.
    #[must_use]
    pub const fn offering(&self) -> Option<OfferingId> {
        self.scope.offering()
    }

    /// Which instructor, when the scope names one.
    #[must_use]
    pub const fn instructor(&self) -> Option<&InstructorName> {
        self.scope.instructor()
    }

    /// Which term, when the scope names one.
    #[must_use]
    pub const fn term(&self) -> Option<&TermCode> {
        self.scope.term()
    }
}

/// The first scope dimension two reviews disagree on, in
/// [`ScopeDimension::ALL`] order.
fn differing_dimension(left: &ReviewScope, right: &ReviewScope) -> Option<ScopeDimension> {
    for dimension in ScopeDimension::ALL {
        let same = match dimension {
            ScopeDimension::Offering => left.offering() == right.offering(),
            ScopeDimension::Instructor => left.instructor() == right.instructor(),
            ScopeDimension::Term => left.term() == right.term(),
            ScopeDimension::Source => left.source() == right.source(),
        };
        if !same {
            return Some(dimension);
        }
    }
    None
}

/// The named ways this crate will combine offerings into a course.
///
/// Section 29.5 requires the aggregation to be *explicit* and the execution
/// plan requires the method to be *named*; neither names the methods. These two
/// are this crate's own, and the contract is that a promotion carries one --
/// not that these are the only two that could ever exist. A third is a new arm
/// here, a new arm of [`CourseReading`], and a new row in the contract, which
/// is the intended cost.
///
/// There is no arm that means "whatever the default is", and the method is not
/// decoration: [`CourseReading`] has one arm per method and the two arms hold
/// structurally different values, so
/// `the_named_method_decides_what_the_course_value_is` runs both over one input
/// and requires the results to differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AggregationMethod {
    /// Every review counts once, in one distribution per dimension. Nothing is
    /// weighted and no offering is privileged.
    PooledBandCounts,
    /// The offerings are kept apart and the course value is the list of them.
    /// Section 34's *단일 score 비기본* is what this arm exists for: a reader
    /// who wants the course sees the offerings it is made of.
    PerOfferingListing,
}

impl AggregationMethod {
    /// Exhaustive listing. Not a precedence order.
    pub const ALL: [Self; 2] = [Self::PooledBandCounts, Self::PerOfferingListing];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PooledBandCounts => "POOLED_BAND_COUNTS",
            Self::PerOfferingListing => "PER_OFFERING_LISTING",
        }
    }
}

/// One offering's contribution, kept whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferingReading {
    scope: ReviewScope,
    sample_size: u32,
    distributions: Vec<BandDistribution>,
}

impl OfferingReading {
    /// Which offering scope.
    #[must_use]
    pub const fn scope(&self) -> &ReviewScope {
        &self.scope
    }

    /// How many reviews it is over.
    #[must_use]
    pub const fn sample_size(&self) -> u32 {
        self.sample_size
    }

    /// Its distributions, in [`ReviewDimension::ALL`] order.
    #[must_use]
    pub fn distributions(&self) -> &[BandDistribution] {
        &self.distributions
    }
}

/// What a course-level value *is*, which depends on the method that made it.
///
/// There is no accessor on [`CourseAggregate`] that returns a distribution
/// without going through this enum, so a caller who wants a number for a course
/// has to say which method's value they are reading -- and then still gets a
/// distribution rather than a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CourseReading {
    /// [`AggregationMethod::PooledBandCounts`]: one distribution per dimension
    /// over every review of every offering.
    Pooled {
        /// In [`ReviewDimension::ALL`] order.
        distributions: Vec<BandDistribution>,
    },
    /// [`AggregationMethod::PerOfferingListing`]: the offerings, kept apart, in
    /// the order the claim asserted them.
    PerOffering {
        /// One entry per promoted offering aggregate.
        offerings: Vec<OfferingReading>,
    },
}

impl CourseReading {
    /// Which method produced this shape.
    ///
    /// A total `match` in the other direction, so the pairing between a method
    /// and a reading is stated twice and
    /// `the_named_method_decides_what_the_course_value_is` compares the two.
    #[must_use]
    pub const fn method(&self) -> AggregationMethod {
        match self {
            Self::Pooled { .. } => AggregationMethod::PooledBandCounts,
            Self::PerOffering { .. } => AggregationMethod::PerOfferingListing,
        }
    }
}

/// The explicit assertion section 29.5 requires before a course-level value
/// exists.
///
/// Private fields, no `Default`, no `Clone`, no `Copy`, and one constructor.
/// [`CourseAggregate::promote`] consumes it, so a claim is spent once and a
/// second promotion needs a second assertion.
#[derive(Debug, PartialEq, Eq)]
pub struct AggregationClaim {
    method: AggregationMethod,
    course: CourseId,
    asserted_over: Vec<ReviewScope>,
}

impl AggregationClaim {
    /// Asserts that these offering scopes are combined this way for this
    /// course.
    ///
    /// The scopes are recorded from the aggregates the caller is promoting, so
    /// a claim always says what it was made over. It is `#[must_use]` because a
    /// claim nobody promotes is an assertion nobody acted on.
    #[must_use]
    pub fn asserting(
        method: AggregationMethod,
        course: CourseId,
        aggregates: &[OfferingAggregate],
    ) -> Self {
        Self {
            method,
            course,
            asserted_over: aggregates
                .iter()
                .map(|aggregate| aggregate.scope().clone())
                .collect(),
        }
    }

    /// Which named method.
    #[must_use]
    pub const fn method(&self) -> AggregationMethod {
        self.method
    }

    /// Which course.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// The scopes the assertion was made over.
    #[must_use]
    pub fn asserted_over(&self) -> &[ReviewScope] {
        &self.asserted_over
    }
}

/// A course-level value, and the assertion that made it one.
///
/// It is not a `Course` property. `academic-curriculum`'s `Course` holds an
/// identifier, a code and a canonical identity; this names a `CourseId` from
/// the outside, the way an aggregate names the thing it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseAggregate {
    course: CourseId,
    method: AggregationMethod,
    over: Vec<ReviewScope>,
    sample_size: u32,
    reading: CourseReading,
    disclosure: BiasDisclosure,
}

impl CourseAggregate {
    /// The one producer of a course-level review value.
    ///
    /// `claim` is consumed. `aggregates` has to be exactly the set the claim
    /// was asserted over -- same scopes, same order, each once -- so a claim
    /// cannot be made about three offerings and spent on a fourth.
    ///
    /// # Errors
    ///
    /// [`ReviewError::NoReviews`] for an empty set,
    /// [`ReviewError::PromotionInputRepeated`] when one scope appears twice,
    /// and [`ReviewError::PromotionScopeMixed`] when the aggregates are not the
    /// ones the claim names.
    pub fn promote(
        claim: AggregationClaim,
        aggregates: &[OfferingAggregate],
        disclosure: BiasDisclosure,
    ) -> Result<Self, ReviewError> {
        if aggregates.is_empty() {
            return Err(ReviewError::NoReviews);
        }
        let scopes: Vec<ReviewScope> = aggregates
            .iter()
            .map(|aggregate| aggregate.scope().clone())
            .collect();
        for (position, scope) in scopes.iter().enumerate() {
            if scopes[..position].contains(scope) {
                return Err(ReviewError::PromotionInputRepeated);
            }
        }
        if scopes != claim.asserted_over() {
            return Err(ReviewError::PromotionScopeMixed);
        }
        let reading = match claim.method() {
            AggregationMethod::PooledBandCounts => CourseReading::Pooled {
                distributions: pooled(aggregates),
            },
            AggregationMethod::PerOfferingListing => CourseReading::PerOffering {
                offerings: aggregates
                    .iter()
                    .map(|aggregate| OfferingReading {
                        scope: aggregate.scope().clone(),
                        sample_size: aggregate.sample_size(),
                        distributions: aggregate.distributions().to_vec(),
                    })
                    .collect(),
            },
        };
        let sample_size = aggregates
            .iter()
            .map(OfferingAggregate::sample_size)
            .fold(0, u32::saturating_add);
        Ok(Self {
            course: claim.course(),
            method: claim.method(),
            over: scopes,
            sample_size,
            reading,
            disclosure,
        })
    }

    /// Which course this is about.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// The named method the claim asserted.
    #[must_use]
    pub const fn method(&self) -> AggregationMethod {
        self.method
    }

    /// The offering scopes it was promoted from.
    #[must_use]
    pub fn over(&self) -> &[ReviewScope] {
        &self.over
    }

    /// How many reviews are behind it.
    #[must_use]
    pub const fn sample_size(&self) -> u32 {
        self.sample_size
    }

    /// The course value, in the shape the named method produces.
    #[must_use]
    pub const fn reading(&self) -> &CourseReading {
        &self.reading
    }

    /// The six disclosures. Always present.
    #[must_use]
    pub const fn disclosure(&self) -> &BiasDisclosure {
        &self.disclosure
    }
}

/// One distribution per dimension over every review of every offering.
fn pooled(aggregates: &[OfferingAggregate]) -> Vec<BandDistribution> {
    let mut distributions = Vec::with_capacity(ReviewDimension::ALL.len());
    for dimension in ReviewDimension::ALL {
        let mut counts = [0_u32; 5];
        for aggregate in aggregates {
            let distribution = aggregate.distribution(dimension);
            for band in DimensionBand::ALL {
                counts[band.index()] =
                    counts[band.index()].saturating_add(distribution.count(band));
            }
        }
        distributions.push(BandDistribution { dimension, counts });
    }
    distributions
}
