//! Section 25.4's four display words, over section 3.9's five statuses.
//!
//! > 졸업 audit의 `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`.
//!
//! # Four here, five there, and the difference is recorded rather than resolved
//!
//! `academic_domain::engines::ProofStatus` is the section 3.9 proof-tree node
//! status and has **five** arms: `SATISFIED`, `NEEDS`, `NOT_SATISFIED`,
//! `UNKNOWN`, `CONFLICT`. `P2-U3`'s engine publishes those five, and section
//! 11.3's own rendered tree shows `NEEDS 12` on one line and `NOT_SATISFIED` on
//! another. Section 25.4 names **four**, and three of them —  `SATISFIED`,
//! `UNKNOWN`, `CONFLICT` — are spelled identically in both.
//!
//! So `REMAINING` covers `NEEDS` and `NOT_SATISFIED`, and that is a reading
//! rather than an invention: the dashboard has to show *some* state for every
//! rule the audit evaluated, section 25.4 offers exactly these four words, and
//! the other three are fixed by their own spelling. Nothing else is available
//! to either of the remaining statuses. `audit_states_are_exactly_four` is what
//! makes that checkable — it reads the four words out of the design document,
//! compares them against [`AuditState::ALL`] in both directions and in order,
//! and then requires [`AuditState::of`] to be **total** over `ProofStatus::ALL`
//! and to send exactly `NEEDS` and `NOT_SATISFIED` to the same word.
//!
//! **What the collapse costs, and what pays it back.** `NEEDS` is a quantified
//! shortfall that an admitted path closes; `NOT_SATISFIED` is a rule no
//! admitted path closes. A reader of the word `REMAINING` alone cannot tell
//! *twelve more credits* from *this course was only ever planned*, and section
//! 11.3 shows both on one screen. So the collapse never happens at the value
//! level: [`AuditStateReading`] keeps the engine's own status and derives the
//! word, [`AuditStateReading::engine_status`] is always available, and there is
//! no constructor that takes an [`AuditState`] directly. The word is what is
//! displayed; the status is what is kept.
//!
//! That discrepancy is written into `docs/contracts/graduation-audit.md` and
//! the pull-request body as the seventh count mismatch measured in this run.

use academic_domain::engines::ProofStatus;

/// One of section 25.4's four graduation-audit display words.
///
/// No `FromStr`, no `TryFrom<&str>`, no `From<&str>` and no arm holding a
/// free-form word: a state this crate cannot name is a state it does not show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuditState {
    /// `SATISFIED` — the rule holds on the frozen inputs.
    Satisfied,
    /// `REMAINING` — the rule does not hold yet.
    ///
    /// The image of both `ProofStatus::Needs` and `ProofStatus::NotSatisfied`.
    /// See this module's header for why, and for what keeps the two apart.
    Remaining,
    /// `UNKNOWN` — an input the rule needs is not known.
    Unknown,
    /// `CONFLICT` — two admitted sources disagree about an input it used.
    Conflict,
}

impl AuditState {
    /// Every state, in section 25.4's own order.
    ///
    /// Enumerated rather than counted. `audit_states_are_exactly_four` reads
    /// section 25.4's line out of the design document and compares the two.
    pub const ALL: [Self; 4] = [
        Self::Satisfied,
        Self::Remaining,
        Self::Unknown,
        Self::Conflict,
    ];

    /// The word section 25.4 spells this state with.
    #[must_use]
    pub const fn spec_word(self) -> &'static str {
        match self {
            Self::Satisfied => "SATISFIED",
            Self::Remaining => "REMAINING",
            Self::Unknown => "UNKNOWN",
            Self::Conflict => "CONFLICT",
        }
    }

    /// The display word for a section 3.9 status.
    ///
    /// Total over `ProofStatus::ALL` by being a `match` with no wildcard arm: a
    /// sixth status stops this crate compiling rather than falling into a
    /// nearest neighbour.
    #[must_use]
    pub const fn of(status: ProofStatus) -> Self {
        match status {
            ProofStatus::Satisfied => Self::Satisfied,
            ProofStatus::Needs | ProofStatus::NotSatisfied => Self::Remaining,
            ProofStatus::Unknown => Self::Unknown,
            ProofStatus::Conflict => Self::Conflict,
        }
    }
}

impl core::fmt::Display for AuditState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.spec_word())
    }
}

/// A displayed audit state that still carries the status it was derived from.
///
/// The one producer is [`AuditStateReading::of`], which takes the engine's own
/// status. There is no constructor taking an [`AuditState`], so a reading whose
/// word is not the image of its status is unrepresentable, and no path through
/// this crate loses the difference between `NEEDS` and `NOT_SATISFIED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuditStateReading {
    status: ProofStatus,
}

impl AuditStateReading {
    /// Reads a section 3.9 status as a section 25.4 display state.
    #[must_use]
    pub const fn of(status: ProofStatus) -> Self {
        Self { status }
    }

    /// The word section 25.4 shows.
    #[must_use]
    pub const fn state(self) -> AuditState {
        AuditState::of(self.status)
    }

    /// The status `P2-U3`'s engine published, kept whatever the word is.
    #[must_use]
    pub const fn engine_status(self) -> ProofStatus {
        self.status
    }

    /// Whether this reading is settled enough to enter a percentage.
    ///
    /// `UNKNOWN` is *필요한 정보가 없음* and `CONFLICT` is two admitted sources
    /// disagreeing; folding either into a ratio manufactures a denominator.
    /// `academic_record::views::GpaValue::Unknown` refuses the same fold one
    /// surface over, and [`crate::SecondaryPercentage`] is where this is used.
    #[must_use]
    pub const fn is_evaluated(self) -> bool {
        match self.status {
            ProofStatus::Satisfied | ProofStatus::Needs | ProofStatus::NotSatisfied => true,
            ProofStatus::Unknown | ProofStatus::Conflict => false,
        }
    }
}
