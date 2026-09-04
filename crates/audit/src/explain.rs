//! Opening a credit number into the reason for every attempt behind it.
//!
//! Section 11.3: *사용자는 숫자뿐 아니라 "왜 이 학점이 포함/제외되었는가"를 열
//! 수 있다.*
//!
//! The drilldown is **total over the transcript**, not over the attempts that
//! counted. A view that listed the included attempts would answer half the
//! sentence: a user asking why a number is lower than expected is asking about
//! the attempts that are *not* in it, and those are the ones a list of
//! inclusions does not contain. [`CreditExplanation::lines`] therefore carries
//! one line per transcript entry, and `credit_explanation_drilldown` asserts
//! the partition rather than the sum.
//!
//! Every exclusion names its reason, and the reason is the record engine's own
//! `DispositionReason` wherever the record engine is what excluded it. This
//! crate adds exactly one reason of its own -- the attempt earned credit and
//! this rule's category is not one it counts under -- because that is the one
//! decision the requirement rule makes and the record engine does not.

use academic_domain::AttemptId;
use academic_record::views::DispositionReason;
use academic_requirement::{CreditCategory, RuleId};

use crate::{
    source::RuleSourceSpan,
    transcript::{EntryAdmission, TranscriptSnapshot, reason_token},
};

/// Why one attempt is or is not inside one credit rule's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditVerdict {
    /// Inside the number, contributing these credits.
    Included {
        /// The credits it contributes.
        credits: u16,
        /// The record engine's reason for earning them.
        reason: DispositionReason,
    },
    /// Outside the number: the record engine earned no credit on it.
    NoCreditEarned {
        /// The record engine's reason.
        reason: DispositionReason,
    },
    /// Outside the number: whether it earns credit is not known.
    ContributionUnknown {
        /// The record engine's reason.
        reason: DispositionReason,
    },
    /// Outside the number: it earned credit under no category this rule counts.
    ///
    /// The one reason this crate adds, and the only one the requirement rule
    /// rather than the record engine decides.
    OutsideCategory,
}

impl CreditVerdict {
    /// Whether this attempt is inside the number.
    #[must_use]
    pub const fn is_included(self) -> bool {
        matches!(self, Self::Included { .. })
    }

    /// The stable token.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Included { .. } => "INCLUDED",
            Self::NoCreditEarned { .. } => "NO_CREDIT_EARNED",
            Self::ContributionUnknown { .. } => "CONTRIBUTION_UNKNOWN",
            Self::OutsideCategory => "OUTSIDE_CATEGORY",
        }
    }

    /// The one-line reason a reader sees.
    #[must_use]
    pub fn reason_text(self) -> String {
        match self {
            Self::Included { credits, reason } => {
                format!("earned {credits} credits ({})", reason_token(reason))
            }
            Self::NoCreditEarned { reason } => {
                format!("earned no credits ({})", reason_token(reason))
            }
            Self::ContributionUnknown { reason } => {
                format!("credit contribution not known ({})", reason_token(reason))
            }
            Self::OutsideCategory => "earned credits under no category this rule counts".to_owned(),
        }
    }
}

/// One attempt's line in one credit rule's drilldown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditLine {
    attempt: AttemptId,
    course_code: String,
    verdict: CreditVerdict,
}

impl CreditLine {
    /// The attempt.
    #[must_use]
    pub const fn attempt(&self) -> AttemptId {
        self.attempt
    }

    /// The course code the transcript printed.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// Why it is or is not inside the number.
    #[must_use]
    pub const fn verdict(&self) -> CreditVerdict {
        self.verdict
    }
}

/// Every attempt behind one credit rule's number, included or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditExplanation {
    rule: RuleId,
    category: CreditCategory,
    source: RuleSourceSpan,
    lines: Vec<CreditLine>,
}

impl CreditExplanation {
    /// Builds the drilldown for one credit rule over the whole transcript.
    ///
    /// One line per entry, in transcript order, with no entry omitted and none
    /// repeated.
    #[must_use]
    pub fn build(
        rule: RuleId,
        category: CreditCategory,
        source: RuleSourceSpan,
        transcript: &TranscriptSnapshot,
    ) -> Self {
        let lines = transcript
            .entries()
            .iter()
            .map(|entry| {
                let verdict = match entry.admission() {
                    EntryAdmission::Counted { credits, reason } => {
                        if entry.categories().contains(&category) {
                            CreditVerdict::Included {
                                credits: credits.get(),
                                reason,
                            }
                        } else {
                            CreditVerdict::OutsideCategory
                        }
                    }
                    EntryAdmission::Excluded { reason } => CreditVerdict::NoCreditEarned { reason },
                    EntryAdmission::Pending { reason } => {
                        CreditVerdict::ContributionUnknown { reason }
                    }
                };
                CreditLine {
                    attempt: entry.attempt(),
                    course_code: entry.course_code().to_owned(),
                    verdict,
                }
            })
            .collect();
        Self {
            rule,
            category,
            source,
            lines,
        }
    }

    /// The rule the number belongs to.
    #[must_use]
    pub const fn rule(&self) -> &RuleId {
        &self.rule
    }

    /// The category it counts.
    #[must_use]
    pub const fn category(&self) -> &CreditCategory {
        &self.category
    }

    /// The official page and paragraph the rule was read from.
    #[must_use]
    pub const fn source(&self) -> &RuleSourceSpan {
        &self.source
    }

    /// One line per transcript entry.
    #[must_use]
    pub fn lines(&self) -> &[CreditLine] {
        &self.lines
    }

    /// The credits the included lines contribute.
    ///
    /// Recomputed from the lines rather than carried beside them, so a
    /// drilldown that disagreed with its own total is not a value.
    #[must_use]
    pub fn included_credits(&self) -> u32 {
        self.lines
            .iter()
            .filter_map(|line| match line.verdict {
                CreditVerdict::Included { credits, .. } => Some(u32::from(credits)),
                _ => None,
            })
            .sum()
    }
}
