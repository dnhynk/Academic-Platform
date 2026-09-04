//! Section 25.2's second line: `최대 1–3개, “왜 지금”과 예상 시간`.
//!
//! # An item without a reason and a time is not constructible
//!
//! [`PrerequisiteItem::offer`] takes the reason and the time as *parameters*.
//! The fields are private, there is no `Default`, no setter, and no builder
//! that could leave one unset, so there is no state in which an item exists
//! and either is missing. `tests/compile_fail/a_prerequisite_item_needs_its_reason_and_its_time.rs`
//! is the compiled half.
//!
//! **The reason is a type, not prose.** `“왜 지금”` asks why *now*, and the
//! answer is the occasion that makes it now — a [`crate::UpcomingUse`], which
//! by construction is strictly ahead of the instant it was judged from. A
//! free-text field would let `왜 지금` be answered with anything at all,
//! including with nothing, and would put user text in a crate that holds none.
//!
//! # The bound is the document's
//!
//! [`crate::LOWEST_BRIEF`] and [`crate::HIGHEST_BRIEF`] are read back out of
//! `최대 1–3개` by `prerequisite_count_is_within_one_to_three_with_reason_and_time`,
//! which splits that phrase on the document's own en dash. A brief is assembled
//! whole by [`PrerequisiteBrief::assemble`] and there is no `push`, so a fourth
//! item cannot be added after the check.

use academic_domain::EntityId;

use crate::{HIGHEST_BRIEF, HomeError, LOWEST_BRIEF, UpcomingUse};

/// `예상 시간`, in whole minutes.
///
/// Minutes rather than a float: section 28 requires every engine output to be
/// reproducible byte for byte, and a binary floating-point duration would
/// depend on the machine that computed it. Zero is refused, because an estimate
/// of no time is not an estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EstimatedMinutes(u16);

impl EstimatedMinutes {
    /// Records an estimate.
    ///
    /// # Errors
    ///
    /// [`HomeError::EstimateIsZero`] for zero.
    pub const fn new(minutes: u16) -> Result<Self, HomeError> {
        if minutes == 0 {
            return Err(HomeError::EstimateIsZero);
        }
        Ok(Self(minutes))
    }

    /// The estimate, in minutes.
    #[must_use]
    pub const fn minutes(self) -> u16 {
        self.0
    }
}

/// One prerequisite offered before a class, with its reason and its time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrerequisiteItem {
    concept: EntityId,
    why_now: UpcomingUse,
    estimated: EstimatedMinutes,
}

impl PrerequisiteItem {
    /// Offers one prerequisite.
    ///
    /// Both of section 25.2's requirements are parameters, so an item that
    /// carries neither cannot be written.
    #[must_use]
    pub const fn offer(
        concept: EntityId,
        why_now: UpcomingUse,
        estimated: EstimatedMinutes,
    ) -> Self {
        Self {
            concept,
            why_now,
            estimated,
        }
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// `왜 지금`: the occasion that makes this needed now.
    #[must_use]
    pub const fn why_now(&self) -> UpcomingUse {
        self.why_now
    }

    /// `예상 시간`.
    #[must_use]
    pub const fn estimated(&self) -> EstimatedMinutes {
        self.estimated
    }
}

/// The one to three prerequisites section 25.2's second line allows.
///
/// Assembled whole. There is no `push`, no `extend`, no `insert` and no
/// `&mut` accessor, so the bound is checked at the only moment a brief comes
/// into existence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrerequisiteBrief {
    items: Vec<PrerequisiteItem>,
}

impl PrerequisiteBrief {
    /// Assembles a brief.
    ///
    /// # Errors
    ///
    /// [`HomeError::PrerequisiteCountOutOfBounds`] when the count is outside
    /// the document's `최대 1–3개` — which includes the empty brief, because a
    /// group with nothing to offer shows no card rather than an empty one.
    pub fn assemble(items: Vec<PrerequisiteItem>) -> Result<Self, HomeError> {
        let count = items.len();
        if !(LOWEST_BRIEF..=HIGHEST_BRIEF).contains(&count) {
            return Err(HomeError::PrerequisiteCountOutOfBounds { count });
        }
        Ok(Self { items })
    }

    /// The items, in the order they were offered.
    #[must_use]
    pub fn items(&self) -> &[PrerequisiteItem] {
        &self.items
    }
}
