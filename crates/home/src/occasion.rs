//! What today holds, and what makes a use *upcoming*.
//!
//! Section 25.2's first line names three things and no fourth:
//! `수업, assessment deadline, project event`. [`ScheduledOccasion`] is those
//! three, read out of that line by `home_group_order_is_stable_one_to_eight`'s
//! neighbour `the_three_occasions_are_section_25_2s_own`.
//!
//! [`UpcomingUse`] is the load-bearing type of this crate. Two of section
//! 25.2's rules turn on it and neither restates the other:
//!
//! * the second line's `“왜 지금”` — a prerequisite is offered *because* an
//!   occasion is coming, and [`crate::PrerequisiteItem`] cannot be built
//!   without one;
//! * the eighth line's `실제 upcoming use가 있을 때만` — a freshness alert is
//!   raised *because* an occasion is coming, and [`crate::FreshnessAlert`]
//!   cannot be built without one.
//!
//! Writing it once is deliberate. Two reasons spelled two ways would let one of
//! the rules relax without the other noticing.

use academic_domain::{EntityId, TimestampMillis};

use crate::HomeError;

/// The three things section 25.2's first line says today actually holds.
///
/// It is a closed set with no `Other`: an occasion this surface cannot name is
/// an occasion it does not show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScheduledOccasion {
    /// `수업` — a class.
    Class,
    /// `assessment deadline`.
    AssessmentDeadline,
    /// `project event`.
    ProjectEvent,
}

impl ScheduledOccasion {
    /// Exhaustive listing, in the order section 25.2's first line names them.
    pub const ALL: [Self; 3] = [Self::Class, Self::AssessmentDeadline, Self::ProjectEvent];

    /// The specification's own words for this occasion.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::Class => "수업",
            Self::AssessmentDeadline => "assessment deadline",
            Self::ProjectEvent => "project event",
        }
    }
}

/// One thing on today's schedule.
///
/// `subject` is the entity the occasion is about — the offering being taught,
/// the assessment falling due, the project the event belongs to. It is an
/// `EntityId` because that is what the registry hands out and what a backlink
/// resolves; this crate invents no second identity for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledItem {
    occasion: ScheduledOccasion,
    subject: EntityId,
    at: TimestampMillis,
}

impl ScheduledItem {
    /// Records one scheduled thing.
    ///
    /// There is no refusal here on purpose: today's schedule shows what is on
    /// it, including what has already happened this morning. The instant is
    /// compared against a window when the screen groups its alerts, and that is
    /// where a boundary belongs.
    #[must_use]
    pub const fn new(occasion: ScheduledOccasion, subject: EntityId, at: TimestampMillis) -> Self {
        Self {
            occasion,
            subject,
            at,
        }
    }

    /// Which of section 25.2's three occasions this is.
    #[must_use]
    pub const fn occasion(&self) -> ScheduledOccasion {
        self.occasion
    }

    /// The entity it is about.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// When it falls.
    #[must_use]
    pub const fn at(&self) -> TimestampMillis {
        self.at
    }
}

/// A use that has not happened yet.
///
/// # There is one constructor and it refuses
///
/// [`Self::declare`] is the only way to obtain one, its fields are private,
/// there is no `Default`, and it derives nothing that could assemble one. An
/// occasion at or before the reference instant is refused with
/// [`HomeError::OccasionIsNotUpcoming`], so a card that needs an upcoming use
/// cannot be built out of a past one by picking a smaller number.
///
/// # What it cannot check
///
/// That the occasion is on a real timetable. This crate has no edge to the
/// surface that would know, and the crate documentation says so rather than
/// letting the type imply more than it verifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpcomingUse {
    occasion: ScheduledOccasion,
    subject: EntityId,
    at: TimestampMillis,
}

impl UpcomingUse {
    /// Declares an occasion as upcoming, against the instant it is judged from.
    ///
    /// # Errors
    ///
    /// [`HomeError::OccasionIsNotUpcoming`] when `at` is not strictly after
    /// `reference`.
    pub fn declare(
        occasion: ScheduledOccasion,
        subject: EntityId,
        at: TimestampMillis,
        reference: TimestampMillis,
    ) -> Result<Self, HomeError> {
        if at <= reference {
            return Err(HomeError::OccasionIsNotUpcoming {
                occasion_at: at,
                reference,
            });
        }
        Ok(Self {
            occasion,
            subject,
            at,
        })
    }

    /// Which of section 25.2's three occasions is coming.
    #[must_use]
    pub const fn occasion(&self) -> ScheduledOccasion {
        self.occasion
    }

    /// The entity the occasion is about.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// When it falls.
    #[must_use]
    pub const fn at(&self) -> TimestampMillis {
        self.at
    }
}
