//! Valid time: the effective-date parser and the two dates it can find.
//!
//! Section 29.2: *a document whose effective date cannot be found is
//! `UNSCOPED_OFFICIAL_SOURCE` and is not automatically published as a rule.*
//! [`Dating`] is that sentence as a type, and it is the type that reaches
//! publication: [`crate::publish`] takes a value only the dated arm can
//! produce, so the refusal is a compile error rather than a check a caller can
//! skip past.
//!
//! # This is valid time, not the clock
//!
//! [`EffectiveDate`] and [`IssuanceDate`] are the document's own dates. The
//! wall clock at retrieval is [`crate::manifest::RetrievalInstant`] and origin
//! order is [`crate::stage::IngestSeq`]. Nothing here converts between them:
//! `CONTRIBUTING.md`'s third rule is that the three axes stay separate in
//! types, and `the_three_time_axes_are_distinct_types` is what executes it.

use core::fmt;

/// The status spelling section 29.2 gives an undated official document.
pub const UNSCOPED_OFFICIAL_SOURCE: &str = "UNSCOPED_OFFICIAL_SOURCE";

/// Why a date was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DateError {
    /// The month was not `1..=12`, or the day was not valid for that month.
    #[error("{year:04}-{month:02}-{day:02} is not a calendar date")]
    NotACalendarDate {
        /// The year as written.
        year: u16,
        /// The month as written.
        month: u8,
        /// The day as written.
        day: u8,
    },
}

/// A calendar date, validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

/// Days in each month of a common year, indexed by month minus one.
const COMMON_YEAR_MONTH_LENGTHS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

const fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

impl Date {
    /// Validates and takes a date.
    ///
    /// # Errors
    ///
    /// [`DateError::NotACalendarDate`] when the month is outside `1..=12` or
    /// the day is outside that month's length.
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, DateError> {
        let invalid = DateError::NotACalendarDate { year, month, day };
        if !(1..=12).contains(&month) {
            return Err(invalid);
        }
        let index = usize::from(month - 1);
        let Some(length) = COMMON_YEAR_MONTH_LENGTHS.get(index).copied() else {
            return Err(invalid);
        };
        let length = if month == 2 && is_leap_year(year) {
            length + 1
        } else {
            length
        };
        if day == 0 || day > length {
            return Err(invalid);
        }
        Ok(Self { year, month, day })
    }

    /// The year.
    #[must_use]
    pub const fn year(self) -> u16 {
        self.year
    }

    /// The month.
    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    /// The day.
    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }

    /// How this date stands to `other`, as a name rather than a number.
    ///
    /// The conflict module compares dates through this, so the comparison it
    /// performs is a named relation and never a subtraction.
    #[must_use]
    pub fn relation_to(self, other: Self) -> DateRelation {
        if self == other {
            DateRelation::Same
        } else if (self.year, self.month, self.day) < (other.year, other.month, other.day) {
            DateRelation::Earlier
        } else {
            DateRelation::Later
        }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// Where one date stands relative to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateRelation {
    /// Strictly before.
    Earlier,
    /// The same day.
    Same,
    /// Strictly after.
    Later,
}

impl DateRelation {
    /// Exhaustive listing.
    pub const ALL: [Self; 3] = [Self::Earlier, Self::Same, Self::Later];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Earlier => "EARLIER",
            Self::Same => "SAME",
            Self::Later => "LATER",
        }
    }
}

/// The day a rule starts to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectiveDate {
    date: Date,
}

impl EffectiveDate {
    /// Takes a validated date as an effective date.
    #[must_use]
    pub const fn on(date: Date) -> Self {
        Self { date }
    }

    /// The date.
    #[must_use]
    pub const fn date(self) -> Date {
        self.date
    }
}

impl fmt::Display for EffectiveDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.date.fmt(formatter)
    }
}

/// The day a document was issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IssuanceDate {
    date: Date,
}

impl IssuanceDate {
    /// Takes a validated date as an issuance date.
    #[must_use]
    pub const fn on(date: Date) -> Self {
        Self { date }
    }

    /// The date.
    #[must_use]
    pub const fn date(self) -> Date {
        self.date
    }
}

impl fmt::Display for IssuanceDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.date.fmt(formatter)
    }
}

/// What the effective-date parser found.
///
/// The two arms are not interchangeable and there is no accessor that turns the
/// second into the first. [`Self::effective_date`] returns `Option`, and the
/// publication path takes the value that only `Some` can build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dating {
    /// The document states when it starts to apply.
    Effective(EffectiveDate),
    /// It does not. Section 29.2's `UNSCOPED_OFFICIAL_SOURCE`.
    Unscoped,
}

impl Dating {
    /// The effective date, when there is one.
    #[must_use]
    pub const fn effective_date(self) -> Option<EffectiveDate> {
        match self {
            Self::Effective(date) => Some(date),
            Self::Unscoped => None,
        }
    }

    /// The status spelling, which is [`UNSCOPED_OFFICIAL_SOURCE`] for the
    /// undated arm.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Effective(_) => "EFFECTIVE_DATED",
            Self::Unscoped => UNSCOPED_OFFICIAL_SOURCE,
        }
    }

    /// Whether this document may be published as a rule at all.
    ///
    /// Advisory. The refusal that matters is the type: [`crate::publish`] takes
    /// a `PublishableRules`, which only the dated arm produces.
    #[must_use]
    pub const fn is_publishable(self) -> bool {
        matches!(self, Self::Effective(_))
    }
}
