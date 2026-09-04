//! Section 25.2's last rule about volume:
//! `알림 수가 많으면 자동 중요도 순으로 숨기지 않고 `Today`, `Soon`,
//! `No deadline`으로 묶는다`.
//!
//! # Grouping is not filtering, and the type is what says so
//!
//! [`GroupedAlerts::group`] takes the whole list by value and returns three
//! lists. It has no count parameter, no threshold, no importance argument and
//! no cut-off, because there is nothing for one to be: every card goes into
//! exactly one bucket and the buckets are handed back whole. A signature with
//! nowhere to put a limit cannot apply one.
//!
//! What is handed back is `&[HomeCard]` in each direction. There is no `&mut`
//! accessor, no owned `Vec` returned, no `drain`, no `truncate`, no `retain`
//! and no `IntoIterator` that yields something smaller than what went in, so a
//! caller cannot shorten a bucket either.
//! `tests/compile_fail/a_grouped_bucket_cannot_be_shortened.rs` is the compiled
//! half.
//!
//! `overflow_is_grouped_not_hidden_and_count_preserved` is the behavioural
//! half, and it compares the union of the three buckets against the input as
//! **multisets in both directions** rather than comparing lengths. A length
//! comparison passes an implementation that dropped one card and duplicated
//! another. Beside it runs a deliberately lossy grouping written in the test,
//! required to fail that same comparison.
//!
//! # The three names are the document's
//!
//! `Today`, `Soon` and `No deadline` are read out of section 25.2's own back
//! quotes and compared with [`AlertBucket::ALL`] in both directions.
//!
//! # Where the boundary between `Today` and `Soon` comes from
//!
//! From the caller. [`DayWindow`] carries the reference instant and the instant
//! the caller says today ends at, because this crate reads no clock and knows
//! no calendar: a day boundary depends on a time zone, and a time zone is not
//! something a screen may guess. A deadline at or before the window's end is
//! `Today` **including one already past** — a deadline that has gone by is the
//! most `Today` thing on the screen, and the alternative would be a fourth
//! bucket the specification does not have.

use academic_domain::TimestampMillis;

use crate::{HomeCard, HomeError};

/// The three groups section 25.2 says a crowded screen is bundled into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AlertBucket {
    /// `Today`.
    Today,
    /// `Soon`.
    Soon,
    /// `No deadline`.
    NoDeadline,
}

impl AlertBucket {
    /// Exhaustive listing, in the order section 25.2 names them.
    pub const ALL: [Self; 3] = [Self::Today, Self::Soon, Self::NoDeadline];

    /// The specification's own words for this bucket.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Soon => "Soon",
            Self::NoDeadline => "No deadline",
        }
    }

    /// Which bucket a deadline falls into.
    ///
    /// Total over both the presence of a deadline and its position, with no
    /// wildcard arm and no fourth answer.
    #[must_use]
    pub const fn of(deadline: Option<TimestampMillis>, window: DayWindow) -> Self {
        match deadline {
            None => Self::NoDeadline,
            Some(at) => {
                if at.value() <= window.ends.value() {
                    Self::Today
                } else {
                    Self::Soon
                }
            }
        }
    }
}

/// The day the caller says it is.
///
/// This crate reads no clock and knows no time zone, so both instants arrive as
/// arguments. That is also why its tests can name the instants they assert
/// against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayWindow {
    starts: TimestampMillis,
    ends: TimestampMillis,
}

impl DayWindow {
    /// Records the window.
    ///
    /// # Errors
    ///
    /// [`HomeError::DayWindowEndsBeforeItStarts`] when the end precedes the
    /// start.
    pub const fn new(starts: TimestampMillis, ends: TimestampMillis) -> Result<Self, HomeError> {
        if ends.value() < starts.value() {
            return Err(HomeError::DayWindowEndsBeforeItStarts {
                start: starts,
                end: ends,
            });
        }
        Ok(Self { starts, ends })
    }

    /// The instant the window is judged from.
    #[must_use]
    pub const fn starts(self) -> TimestampMillis {
        self.starts
    }

    /// The instant the caller says today ends at.
    #[must_use]
    pub const fn ends(self) -> TimestampMillis {
        self.ends
    }
}

/// Every card, in exactly one of section 25.2's three buckets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedAlerts {
    today: Vec<HomeCard>,
    soon: Vec<HomeCard>,
    no_deadline: Vec<HomeCard>,
}

impl GroupedAlerts {
    /// Puts every card into exactly one bucket.
    ///
    /// Consumes the whole input. Nothing is dropped, nothing is ranked and
    /// nothing is capped, because there is no parameter that could ask for any
    /// of those.
    #[must_use]
    pub fn group(cards: Vec<HomeCard>, window: DayWindow) -> Self {
        let mut grouped = Self {
            today: Vec::new(),
            soon: Vec::new(),
            no_deadline: Vec::new(),
        };
        for card in cards {
            match AlertBucket::of(card.deadline(), window) {
                AlertBucket::Today => grouped.today.push(card),
                AlertBucket::Soon => grouped.soon.push(card),
                AlertBucket::NoDeadline => grouped.no_deadline.push(card),
            }
        }
        grouped
    }

    /// One bucket, whole.
    #[must_use]
    pub fn bucket(&self, bucket: AlertBucket) -> &[HomeCard] {
        match bucket {
            AlertBucket::Today => &self.today,
            AlertBucket::Soon => &self.soon,
            AlertBucket::NoDeadline => &self.no_deadline,
        }
    }

    /// How many cards the three buckets hold between them.
    ///
    /// Derived from the buckets rather than remembered from the input, so a
    /// count that disagreed with the buckets is not expressible.
    #[must_use]
    pub fn total(&self) -> usize {
        AlertBucket::ALL
            .into_iter()
            .map(|bucket| self.bucket(bucket).len())
            .sum()
    }
}
