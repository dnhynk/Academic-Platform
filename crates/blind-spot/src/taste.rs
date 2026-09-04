//! Section 23's `탐색을 원할 때만 작은 taste path—한 강의, 한 chapter, 한 toy
//! experiment—를 제공한다`.
//!
//! The count is not a number this file chose.
//! `explore_creates_one_bounded_taste_path` reads the run between the em dashes
//! back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits it
//! on its own `, ` separators and compares the three against [`TASTE_STEPS`] in
//! both directions.
//!
//! ## Bounded is a shape, not a limit
//!
//! A [`TastePath`] holds **one** [`TasteStep`] and not a list, so a path of two
//! lectures is not a value this crate refuses at the end of a constructor — it
//! is a value that cannot be written. Section 23's own three are each already
//! singular: `한 강의`, `한 chapter`, `한 toy experiment`.
//!
//! ## Only the user opens one
//!
//! Section 25.12: `EXPLORE를 누른 경우에만 작은 입문 path를 만든다`.
//! [`TastePath::for_explore`] takes a
//! [`crate::disposition::UserDispositionChoice`], which ADR-003's matrix refuses
//! every automatic actor, and refuses the other three dispositions — so a path
//! the product opened on the user's behalf would need a choice no model run can
//! mint and a disposition the user did not pick.

use serde::{Deserialize, Serialize};

use academic_domain::EntityId;

use crate::{
    BlindSpotError,
    disposition::{UserDisposition, UserDispositionChoice},
};

/// Section 23's three taste steps, in the order it writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TasteStep {
    /// `한 강의`.
    OneLecture,
    /// `한 chapter`.
    OneChapter,
    /// `한 toy experiment`.
    OneToyExperiment,
}

/// The three, in section 23's own order.
pub const TASTE_STEPS: [TasteStep; 3] = [
    TasteStep::OneLecture,
    TasteStep::OneChapter,
    TasteStep::OneToyExperiment,
];

impl TasteStep {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneLecture => "ONE_LECTURE",
            Self::OneChapter => "ONE_CHAPTER",
            Self::OneToyExperiment => "ONE_TOY_EXPERIMENT",
        }
    }

    /// The design document's own spelling for this step.
    #[must_use]
    pub const fn design_token(self) -> &'static str {
        match self {
            Self::OneLecture => "한 강의",
            Self::OneChapter => "한 chapter",
            Self::OneToyExperiment => "한 toy experiment",
        }
    }
}

/// One bounded taste path: one step, about one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TastePath {
    key: EntityId,
    step: TasteStep,
}

impl TastePath {
    /// Opens the one step the user asked for.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::TastePathNeedsExplore`] for any disposition but
    /// `EXPLORE`, and [`BlindSpotError::TastePathIsAboutAnotherField`] when the
    /// choice was recorded for a different key.
    pub fn for_explore(
        choice: &UserDispositionChoice,
        key: EntityId,
        step: TasteStep,
    ) -> Result<Self, BlindSpotError> {
        if choice.disposition() != UserDisposition::Explore {
            return Err(BlindSpotError::TastePathNeedsExplore);
        }
        if choice.field() != key {
            return Err(BlindSpotError::TastePathIsAboutAnotherField);
        }
        Ok(Self { key, step })
    }

    /// Which key it explores.
    #[must_use]
    pub const fn key(self) -> EntityId {
        self.key
    }

    /// The one step.
    #[must_use]
    pub const fn step(self) -> TasteStep {
        self.step
    }
}
