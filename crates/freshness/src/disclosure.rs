//! What a `STALE` band says to the person it is about.
//!
//! Section 34.2's knowledge-state table has a row for
//! `Knowledge Freshness를 실력 저하로 오인`, and its `불확실성 표시` cell is a
//! sentence rather than a widget: `“과거 mastery 유지, 최근 사용 근거 없음”
//! 문구`. Section 13.3's example block says the same thing at length. Both are
//! kept here as the design document's own text, and
//! `stale_copy_says_past_mastery_remains` reads both back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`.
//!
//! ## Three things this type refuses to be able to say
//!
//! * **It cannot appear on a band that is not `STALE`.**
//!   [`StaleDisclosure::of`] answers `None` for the other five, so there is no
//!   value of this type on a fresh concept.
//! * **It cannot say the user does not know something.** The design document's
//!   own `Action` line ends `“모름”으로 표시하지 않음`, and
//!   [`JUDGEMENT_TOKENS`] is the list of spellings the copy is checked against —
//!   with a control, because a token list that matches nothing is the empty
//!   guard `docs/contracts/policy-source-scans.md` records this repository
//!   finding again and again.
//! * **It cannot say anything about mastery at all.** This crate has no name for
//!   a mastery level; the `Mastery:` line of section 13.3's block is the
//!   assertion's own and is rendered by whoever holds the assertion. What is
//!   here is the freshness half — `Recent use`, `Freshness`, `Meaning`,
//!   `Action` — and [`STALE_MEANING`] is the sentence that says the other half
//!   survives.

use academic_domain::FreshnessBand;
use serde::{Deserialize, Serialize};

use crate::projection::FreshnessProjection;

/// Section 13.3's `Meaning:` line, verbatim.
pub const STALE_MEANING: &str = "과거 이해 evidence는 유지되지만 즉시 인출 가능성은 검증되지 않음";

/// Section 13.3's `Action:` line, verbatim.
pub const STALE_ACTION: &str = "필요 시 15분 retrieval check 제안; “모름”으로 표시하지 않음";

/// Section 34.2's `불확실성 표시` cell for this failure, verbatim.
pub const STALE_DISCLOSURE: &str = "과거 mastery 유지, 최근 사용 근거 없음";

/// Section 13.3's own label for a concept with no use in the recent window.
pub const NO_RECENT_USE: &str = "none";

/// Spellings that would turn a statement about retrieval into one about the
/// person.
///
/// The first is the design document's own — its `Action` line forbids exactly
/// that word. The rest are the English and Korean readings of it that a
/// translation or a rewrite would reach for.
/// `stale_copy_says_past_mastery_remains` checks the copy against all of them
/// **and** checks the same reader against a string that contains one, so the
/// zero it reports is a measurement rather than a reader that always answers
/// zero.
pub const JUDGEMENT_TOKENS: [&str; 8] = [
    "모름",
    "모릅니다",
    "잊음",
    "잊어버",
    "does not know",
    "unknown",
    "forgotten",
    "lost",
];

/// The user-facing copy for a `STALE` concept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleDisclosure {
    recent_use: &'static str,
    freshness: &'static str,
    meaning: &'static str,
    action: &'static str,
    disclosure: &'static str,
}

impl StaleDisclosure {
    /// The copy for `projection`, or `None` when the band is not `STALE`.
    #[must_use]
    pub fn of(projection: &FreshnessProjection) -> Option<Self> {
        if projection.band() != FreshnessBand::Stale {
            return None;
        }
        Some(Self {
            recent_use: NO_RECENT_USE,
            freshness: crate::band::band_token(FreshnessBand::Stale),
            meaning: STALE_MEANING,
            action: STALE_ACTION,
            disclosure: STALE_DISCLOSURE,
        })
    }

    /// Section 13.3's `Recent use:` value.
    #[must_use]
    pub const fn recent_use(&self) -> &'static str {
        self.recent_use
    }

    /// Section 13.3's `Freshness:` value.
    #[must_use]
    pub const fn freshness(&self) -> &'static str {
        self.freshness
    }

    /// Section 13.3's `Meaning:` line.
    #[must_use]
    pub const fn meaning(&self) -> &'static str {
        self.meaning
    }

    /// Section 13.3's `Action:` line.
    #[must_use]
    pub const fn action(&self) -> &'static str {
        self.action
    }

    /// Section 34.2's `불확실성 표시` phrase.
    #[must_use]
    pub const fn disclosure(&self) -> &'static str {
        self.disclosure
    }

    /// Every line of the copy, which is what a check for a forbidden spelling
    /// has to read.
    #[must_use]
    pub const fn lines(&self) -> [&'static str; 5] {
        [
            self.recent_use,
            self.freshness,
            self.meaning,
            self.action,
            self.disclosure,
        ]
    }
}
