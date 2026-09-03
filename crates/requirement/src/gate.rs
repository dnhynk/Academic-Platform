//! The four section 38 cells this task leaves open, stated where they bite.
//!
//! None has a default and none is given one here, exactly as
//! `academic_curriculum::gate`, `academic_ingestion::gate` and
//! `academic_consent::gate` do it. What this module supplies is the shape of
//! each cell and the value that stands while it is empty.
//!
//! There is deliberately no cohort table, no thesis-scope table, no
//! double-counting table and no external-recognition table. Each is an official
//! fact the user has to confirm, and a rule that guessed one would be a
//! graduation verdict manufactured out of nothing -- which is the exact failure
//! section 11.1 forbids when it says an absent selector input returns
//! `INDETERMINATE` and never an arbitrary choice.

use crate::dsl::{Applicability, DoubleCountingPolicy, RecognitionPolicy};

/// A section 38 cell this task leaves for the user to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum OpenGate {
    /// `GATE-38-011`: which cohort a rule applies to, and the transitional
    /// arrangement between two standards.
    CohortApplicability,
    /// `GATE-38-012`: the exact scope of the 2027-1 thesis-research rule.
    ThesisRuleScope,
    /// `GATE-38-015`: whether one attempt may count toward two majors at once.
    MultiMajorDoubleCounting,
    /// `GATE-38-016`: how much external, transferred or exchange credit is
    /// recognized.
    ExternalCreditRecognition,
}

impl OpenGate {
    /// All four cells.
    pub const ALL: [Self; 4] = [
        Self::CohortApplicability,
        Self::ThesisRuleScope,
        Self::MultiMajorDoubleCounting,
        Self::ExternalCreditRecognition,
    ];

    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::CohortApplicability => "GATE-38-011",
            Self::ThesisRuleScope => "GATE-38-012",
            Self::MultiMajorDoubleCounting => "GATE-38-015",
            Self::ExternalCreditRecognition => "GATE-38-016",
        }
    }

    /// What the cell leaves open, and what stands while it is empty.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::CohortApplicability => {
                "which admission cohort a rule applies to, and the transitional \
                 arrangement between two standards, is an official fact the \
                 user must confirm (GATE-38-011); a rule scoped by admission \
                 year against an unrecorded year evaluates to UNKNOWN, and no \
                 cohort is assumed from a term, an attempt, or a sibling rule"
            }
            Self::ThesisRuleScope => {
                "the exact scope and transitional arrangement of the 2027-1 \
                 thesis-research requirement needs a departmental notice and an \
                 administrative confirmation (GATE-38-012); an unresolved \
                 applicability evaluates to UNKNOWN, never to satisfied and \
                 never to not-satisfied"
            }
            Self::MultiMajorDoubleCounting => {
                "whether one attempt may count toward two majors at once is an \
                 official fact the user must confirm (GATE-38-015); a \
                 MUTUALLY_EXCLUSIVE rule with no confirmed ceiling evaluates to \
                 UNKNOWN, and nothing infers a ceiling from the member count"
            }
            Self::ExternalCreditRecognition => {
                "how much external, transferred or exchange credit is \
                 recognized is an official fact the user must confirm \
                 (GATE-38-016); a MAXIMUM_RECOGNITION rule with no confirmed \
                 cap evaluates to UNKNOWN, and nothing infers a cap from the \
                 credits presented"
            }
        }
    }
}

/// Every value in this crate that means "no official record exists".
///
/// Enumerated rather than counted, and each entry is the type's own spelling,
/// so a variant renamed on one side fails against the other.
/// `an_absent_official_fact_reads_unknown` walks this list and requires each
/// spelling to be the reading the corresponding rule returns.
#[must_use]
pub fn unknown_readings() -> [(&'static str, &'static str); 3] {
    [
        ("Applicability", applicability_unknown()),
        ("RecognitionPolicy", recognition_unknown()),
        ("DoubleCountingPolicy", double_counting_unknown()),
    ]
}

const fn applicability_unknown() -> &'static str {
    match Applicability::Unknown {
        Applicability::Unknown => "UNKNOWN",
        Applicability::FromAdmissionYear(_) | Applicability::BeforeAdmissionYear(_) => "",
    }
}

const fn recognition_unknown() -> &'static str {
    match RecognitionPolicy::Unknown {
        RecognitionPolicy::Unknown => "UNKNOWN",
        RecognitionPolicy::CappedAt(_) => "",
    }
}

const fn double_counting_unknown() -> &'static str {
    match DoubleCountingPolicy::Unknown {
        DoubleCountingPolicy::Unknown => "UNKNOWN",
        DoubleCountingPolicy::AtMost(_) => "",
    }
}
