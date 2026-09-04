//! The seven section 38 cells this task leaves open, stated where they bite.
//!
//! `t068` section 5's `P2-U3` entry: *Leaves `GATE-38-001`-`GATE-38-004`,
//! `GATE-38-006`, `GATE-38-011`, `GATE-38-012` open as blocking user or
//! official inputs; the engine must display them as the exact missing checks
//! rather than assuming a cohort.*
//!
//! None has a default and none is given one here. The four this crate
//! introduces -- `GATE-38-001` through `GATE-38-004` -- are section 38.1's
//! first four lines, which are exactly the profile fields section 11.1's
//! selector needs. `GATE-38-006` is section 38.1's sixth. `GATE-38-011` and
//! `GATE-38-012` are section 38.2's first two and are already
//! `academic_requirement::OpenGate`'s; this module does not restate their
//! meaning, it forwards the value the rule verdict carried.
//!
//! There is deliberately no cohort table, no standard table, no degree-mode
//! table and no recognition table. Each is a fact the user has to supply, and
//! a selector that guessed one would be a graduation verdict manufactured out
//! of nothing -- which is the exact failure section 11.1 forbids when it says
//! an absent selector input returns `INDETERMINATE` and never an arbitrary
//! choice.

use academic_requirement::OpenGate as RuleGate;

/// A section 38 cell the graduation audit leaves for the user to fill.
///
/// The five `Profile*` cells are section 38.1's own lines, in its own order.
/// [`OpenGate::from_rule_gate`] carries `academic-requirement`'s two forward
/// rather than declaring a second spelling of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum OpenGate {
    /// `GATE-38-001`: the user's admission year.
    ProfileAdmissionYear,
    /// `GATE-38-002`: the curriculum or graduation standard the user selected.
    ProfileGraduationStandard,
    /// `GATE-38-003`: single major, double major, or another degree mode.
    ProfileDegreeMode,
    /// `GATE-38-004`: any additional major or minor.
    ProfileAdditionalMajor,
    /// `GATE-38-006`: transferred and exchange credits, with their recognition
    /// decisions.
    ProfileExchangeOrTransfer,
    /// `GATE-38-011`: which admission cohort a *rule* applies to, and the
    /// transitional arrangement between two standards.
    ///
    /// Distinct from [`OpenGate::ProfileAdmissionYear`], which is the year the
    /// user entered under. Knowing the student's year does not say which
    /// cohort an official rule was written for; section 38.2's first bullet
    /// asks for the applicability rule itself.
    RuleCohortApplicability,
    /// `GATE-38-012`: the exact scope of the 2027-1 thesis-research rule.
    RuleThesisScope,
}

impl OpenGate {
    /// Every cell, in section 38's own order.
    ///
    /// Enumerated rather than counted: `the_open_gates_are_section_38s_own`
    /// reads section 38.1's block and section 38.2's list out of the design
    /// document and requires each entry below to be the line it quotes.
    pub const ALL: [Self; 7] = [
        Self::ProfileAdmissionYear,
        Self::ProfileGraduationStandard,
        Self::ProfileDegreeMode,
        Self::ProfileAdditionalMajor,
        Self::ProfileExchangeOrTransfer,
        Self::RuleCohortApplicability,
        Self::RuleThesisScope,
    ];

    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::ProfileAdmissionYear => "GATE-38-001",
            Self::ProfileGraduationStandard => "GATE-38-002",
            Self::ProfileDegreeMode => "GATE-38-003",
            Self::ProfileAdditionalMajor => "GATE-38-004",
            Self::ProfileExchangeOrTransfer => "GATE-38-006",
            Self::RuleCohortApplicability => "GATE-38-011",
            Self::RuleThesisScope => "GATE-38-012",
        }
    }

    /// The section 38.1 or 38.2 line this cell is, verbatim.
    ///
    /// Compared against the design document by
    /// `the_open_gates_are_section_38s_own`, so a paraphrase fails.
    #[must_use]
    pub const fn spec_line(self) -> &'static str {
        match self {
            Self::ProfileAdmissionYear => "Admission Year",
            Self::ProfileGraduationStandard => "Selected Curriculum/Graduation Standard",
            Self::ProfileDegreeMode => "Degree Mode",
            Self::ProfileAdditionalMajor => "Additional Major / Minor",
            Self::ProfileExchangeOrTransfer => "Transferred/Exchange Credits",
            Self::RuleCohortApplicability => {
                "사용 학번과 선택 가능한 졸업기준의 정확한 적용 규칙·경과조치."
            }
            Self::RuleThesisScope => {
                "2027-1 `컴퓨터공학 학사논문연구` 필수의 적용 대상과 기존 학번 경과조치."
            }
        }
    }

    /// What the user has to supply, and what stands while it is empty.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::ProfileAdmissionYear => {
                "the admission year the user entered under is a user input \
                 (GATE-38-001); without it no requirement set is selected, the \
                 audit is INDETERMINATE, and no cohort is assumed from a term, \
                 an attempt, or a published set"
            }
            Self::ProfileGraduationStandard => {
                "which graduation standard the user lawfully selected is a user \
                 input (GATE-38-002); without it no requirement set is \
                 selected, and the admission year is not read as the standard"
            }
            Self::ProfileDegreeMode => {
                "whether the user is on a single major, a double major, a \
                 minor, a united or a linked programme is a user input \
                 (GATE-38-003); without it no requirement set is selected, and \
                 single major is not assumed"
            }
            Self::ProfileAdditionalMajor => {
                "which additional majors or minors the user carries is a user \
                 input (GATE-38-004); an unanswered question is not an empty \
                 list, so the audit is INDETERMINATE until the user says which \
                 -- including that there are none"
            }
            Self::ProfileExchangeOrTransfer => {
                "which transferred or exchange credits the user carries, and \
                 the recognition decision on each, is a user input \
                 (GATE-38-006); an attempt whose recognition is undecided is \
                 known to be undecided and is never counted as recognized or \
                 as refused"
            }
            Self::RuleCohortApplicability => {
                "which admission cohort a rule applies to, and the transitional \
                 arrangement between two standards, is an official fact \
                 (GATE-38-011); the rule verdict reads UNKNOWN and the leaf \
                 carries this cell"
            }
            Self::RuleThesisScope => {
                "the exact scope and transitional arrangement of the 2027-1 \
                 thesis-research requirement needs a departmental notice and an \
                 administrative confirmation (GATE-38-012); the rule verdict \
                 reads UNKNOWN whatever the record holds, including a completed \
                 thesis"
            }
        }
    }

    /// Carries one of `academic-requirement`'s cells forward.
    ///
    /// `GATE-38-015` and `GATE-38-016` are that crate's and stay that crate's:
    /// a `MUTUALLY_EXCLUSIVE` or `MAXIMUM_RECOGNITION` verdict that reads
    /// `UNKNOWN` is still an `UNKNOWN` leaf here and still blocks
    /// `DETERMINATE`, but this crate does not restate what they leave open, so
    /// they map to `None` and the leaf carries the rule crate's own value.
    #[must_use]
    pub const fn from_rule_gate(gate: RuleGate) -> Option<Self> {
        match gate {
            RuleGate::CohortApplicability => Some(Self::RuleCohortApplicability),
            RuleGate::ThesisRuleScope => Some(Self::RuleThesisScope),
            RuleGate::MultiMajorDoubleCounting | RuleGate::ExternalCreditRecognition => None,
            // `academic_requirement::OpenGate` is `#[non_exhaustive]`, so a
            // wildcard is required here and a cell added there arrives as
            // `None`. That is the fail-closed direction: the leaf still
            // carries the rule crate's own value through
            // `ProofLeaf::rule_gate`, the verdict is still `UNKNOWN`, and this
            // crate simply does not claim to have restated the new cell.
            _ => None,
        }
    }
}
