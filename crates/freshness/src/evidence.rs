//! Section 13.3's first two inputs: when the last strong evidence was, and how
//! often and how spaced the recent ones are.
//!
//! ## Freshness reads only evidence that passed section 13.4's four checks
//!
//! [`DatedEvidence`] wraps an `academic_knowledge_state::EligibleEvidence` and
//! there is no other constructor. That is not a convenience — it is the same
//! rule `P2-N2` holds for promotion, one axis over. Evidence blocked for absent
//! authorship, an unknown outcome or a broken source integrity cannot promote a
//! concept; if it could still *freshen* one, the concept would read
//! `VERY_HIGH` on the strength of evidence the system already refused, and the
//! band a user reads would be the one number the eligibility gate does not
//! cover. There is no value of a type this crate accepts that carries a blocked
//! item.
//!
//! A [`DatedEvidence`] also cannot lie about which concept it is about: the
//! concept comes off the eligible evidence rather than from the caller, so the
//! only mistake left is offering it under another concept's projection, and
//! [`crate::projection::project`] refuses that with its own error.
//!
//! ## Repetition is counted in distinct days, not in items
//!
//! Section 13.3's second input is `최근 일정 window의 반복 횟수와 간격` — count
//! **and** interval. Four items on one afternoon are one occasion and four items
//! across four months are four, so [`Repetition`] counts distinct days and keeps
//! both numbers. A count alone would make a single clustered session look like
//! spaced practice, which is the reading section 13.3 names the interval to
//! prevent.

use academic_domain::{EntityId, EvidenceId, TimestampMillis};
use academic_knowledge_state::{EligibleEvidence, EvidenceKind};

use crate::persistence::{DAY_MILLIS, PersistenceWindow, RetentionPrior, elapsed_millis};

/// One admitted evidence item with the instant it happened.
///
/// Section 13.3's `마지막 strong evidence의 시점과 종류` is exactly these two
/// things, and this is the only shape this crate reads them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatedEvidence {
    evidence: EligibleEvidence,
    occurred_at: TimestampMillis,
}

impl DatedEvidence {
    /// Dates one admitted item.
    ///
    /// The concept is the item's own, so this cannot be built about a concept
    /// the eligibility gate did not link it to.
    #[must_use]
    pub const fn at(evidence: EligibleEvidence, occurred_at: TimestampMillis) -> Self {
        Self {
            evidence,
            occurred_at,
        }
    }

    /// The admitted item, unchanged.
    #[must_use]
    pub const fn evidence(&self) -> &EligibleEvidence {
        &self.evidence
    }

    /// Which concept the eligibility gate linked it to.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.evidence.concept()
    }

    /// Which of section 13.2's rows.
    #[must_use]
    pub const fn kind(&self) -> EvidenceKind {
        self.evidence.kind()
    }

    /// Which evidence item.
    #[must_use]
    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence.evidence_id()
    }

    /// When it happened.
    #[must_use]
    pub const fn occurred_at(&self) -> TimestampMillis {
        self.occurred_at
    }

    /// The window this item's kind decays over under `prior`.
    ///
    /// A `DatedEvidence` holds an `EligibleEvidence`, whose kind comes from
    /// `ConceptEvidence::kind` — which has no `CourseGrade` arm, because section
    /// 13.2's eighth row has no concept to attach an instant to. The fallback is
    /// therefore unreachable and is the shortest window rather than a panic;
    /// `no_dated_evidence_can_carry_a_grade` observes the branch is empty over
    /// every constructible value.
    #[must_use]
    pub fn window(&self, prior: &RetentionPrior) -> PersistenceWindow {
        prior.window_for(self.kind()).unwrap_or(prior.shortest())
    }
}

/// Section 13.3's `반복 횟수와 간격`, over one recent window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repetition {
    occasions: u32,
    span_days: u32,
}

impl Repetition {
    /// Counts distinct days carrying evidence within `window` before `as_of`.
    ///
    /// Items outside the window are not counted, which is what makes this the
    /// *recent* window section 13.3 asks for rather than the whole history.
    #[must_use]
    pub fn over(
        dated: &[DatedEvidence],
        window: PersistenceWindow,
        as_of: TimestampMillis,
    ) -> Self {
        let mut days: Vec<i64> = dated
            .iter()
            .filter_map(|item| {
                let elapsed = elapsed_millis(item.occurred_at(), as_of)?;
                if elapsed > window.millis() {
                    return None;
                }
                item.occurred_at().value().checked_div_euclid(DAY_MILLIS)
            })
            .collect();
        days.sort_unstable();
        days.dedup();
        let span = match (days.first(), days.last()) {
            (Some(first), Some(last)) => last.saturating_sub(*first),
            _ => 0,
        };
        Self {
            occasions: u32::try_from(days.len()).unwrap_or(u32::MAX),
            span_days: u32::try_from(span).unwrap_or(u32::MAX),
        }
    }

    /// How many distinct days carried evidence.
    #[must_use]
    pub const fn occasions(self) -> u32 {
        self.occasions
    }

    /// How many days separate the first and the last of them.
    #[must_use]
    pub const fn span_days(self) -> u32 {
        self.span_days
    }

    /// How many repeats beyond the first occasion there were.
    ///
    /// This is the multiplier section 13.3's second input contributes: a single
    /// occasion extends nothing, and each further **day** extends the window it
    /// applies to by one more of itself. Two items an hour apart are one
    /// occasion and extend nothing, which is the count-versus-interval
    /// distinction the bullet names.
    #[must_use]
    pub const fn repeats(self) -> u32 {
        self.occasions.saturating_sub(1)
    }
}
