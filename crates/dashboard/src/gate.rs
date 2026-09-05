//! The section 38 cells this surface leaves open, stated where they bite.
//!
//! `t068` section 5's `P2-X3` entry: *Leaves `GATE-38-017` open (per-term
//! offering facts) and surfaces `GATE-38-001`–`GATE-38-006` as blocking
//! dashboard inputs.*
//!
//! # Which lines those identifiers are
//!
//! Section 38.1 is a block of ten lines and section 38.2 is a list of eleven
//! bullets, and the identifiers run through them in order: `GATE-38-001`
//! through `GATE-38-010` are section 38.1's ten, and `GATE-38-011` through
//! `GATE-38-021` are section 38.2's eleven. `the_open_gates_are_section_38s_own`
//! derives every identifier here from that arithmetic and from the line's own
//! position rather than from a table, so a renumbered section or a reordered
//! line fails instead of silently renaming a cell. `academic-audit` and
//! `academic-offering` each derive their own the same way.
//!
//! The six blocking cells are therefore section 38.1's **first six** lines:
//! the admission year, the selected graduation standard, the degree mode, any
//! additional major, the official transcript, and the transferred and exchange
//! credits with their recognition decisions. `GATE-38-017` is section 38.2's
//! **seventh** bullet, 해당 학기의 최신 CourseOffering, 교수자, 정원, 시간표,
//! syllabus, 평가 방식.
//!
//! # What overlaps with `P2-U3`, and the one cell that does not
//!
//! `academic_audit::OpenGate` holds five of these six — `GATE-38-001` through
//! `GATE-38-004` and `GATE-38-006` — plus two rule-side cells this surface does
//! not introduce. It does **not** hold `GATE-38-005`, and correctly: the
//! graduation engine takes an attempt set as a frozen input and never reads a
//! transcript. The dashboard does — every average on it is over the imported
//! record — so the transcript is a blocking input here and is not one there.
//! That is why this enumeration is derived from section 38.1 and not forwarded
//! from that crate: forwarding would have carried the hole with it.
//!
//! `academic-audit` is a **dev** edge for exactly this reason.
//! `the_open_gates_are_section_38s_own` compares the five shared cells against
//! that crate's own enumeration after each was read out of the design document,
//! which is a comparison; a product edge would have made one crate's answer the
//! other's by construction. `academic-offering` is a dev edge on the same terms
//! for `GATE-38-017`.
//!
//! # None of them is filled here
//!
//! There is no admission-year table, no standard table, no degree-mode table,
//! no recognition table and no offering forecast. Each is a fact the user or
//! the school has to supply, and a dashboard that guessed one would publish an
//! average, a credit total and a graduation percentage manufactured out of
//! nothing.

/// A section 38 cell this surface leaves for the user or the school to fill.
///
/// The six `Profile*` cells block the dashboard: with any of them empty the
/// screen shows the exact missing check rather than a number.
/// [`OpenGate::CurrentTermOfferingFacts`] does not block — it stays open every
/// term and is what the planner's left rail is a reading *of*.
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
    /// `GATE-38-005`: the current official transcript.
    ///
    /// Not in `academic_audit::OpenGate`, and not an omission there: the
    /// graduation engine reads an attempt set it is handed. Every average on
    /// this screen is over the imported record, so the import is a blocking
    /// input here.
    ProfileOfficialTranscript,
    /// `GATE-38-006`: transferred and exchange credits, with their recognition
    /// decisions.
    ProfileExchangeOrTransfer,
    /// `GATE-38-017`: this term's offerings, instructors, capacity, timetable,
    /// syllabus and assessment scheme.
    ///
    /// Left open rather than closed. `P2-U5` states the same cell for the
    /// forecast; here it is what makes a [`crate::CandidateOffering`] a
    /// caller-supplied official reading rather than something this crate knows.
    CurrentTermOfferingFacts,
}

impl OpenGate {
    /// Every cell, in section 38's own order.
    ///
    /// Enumerated rather than counted.
    pub const ALL: [Self; 7] = [
        Self::ProfileAdmissionYear,
        Self::ProfileGraduationStandard,
        Self::ProfileDegreeMode,
        Self::ProfileAdditionalMajor,
        Self::ProfileOfficialTranscript,
        Self::ProfileExchangeOrTransfer,
        Self::CurrentTermOfferingFacts,
    ];

    /// The six cells that block the dashboard.
    pub const BLOCKING: [Self; 6] = [
        Self::ProfileAdmissionYear,
        Self::ProfileGraduationStandard,
        Self::ProfileDegreeMode,
        Self::ProfileAdditionalMajor,
        Self::ProfileOfficialTranscript,
        Self::ProfileExchangeOrTransfer,
    ];

    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::ProfileAdmissionYear => "GATE-38-001",
            Self::ProfileGraduationStandard => "GATE-38-002",
            Self::ProfileDegreeMode => "GATE-38-003",
            Self::ProfileAdditionalMajor => "GATE-38-004",
            Self::ProfileOfficialTranscript => "GATE-38-005",
            Self::ProfileExchangeOrTransfer => "GATE-38-006",
            Self::CurrentTermOfferingFacts => "GATE-38-017",
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
            Self::ProfileOfficialTranscript => "Current Official Transcript",
            Self::ProfileExchangeOrTransfer => "Transferred/Exchange Credits",
            Self::CurrentTermOfferingFacts => {
                "해당 학기의 최신 CourseOffering, 교수자, 정원, 시간표, syllabus, 평가 방식."
            }
        }
    }

    /// Whether an empty cell stops the dashboard showing a number.
    #[must_use]
    pub const fn blocks_the_dashboard(self) -> bool {
        match self {
            Self::ProfileAdmissionYear
            | Self::ProfileGraduationStandard
            | Self::ProfileDegreeMode
            | Self::ProfileAdditionalMajor
            | Self::ProfileOfficialTranscript
            | Self::ProfileExchangeOrTransfer => true,
            Self::CurrentTermOfferingFacts => false,
        }
    }

    /// What is missing, and what stands while it is.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::ProfileAdmissionYear => {
                "the admission year the user entered under is a user input \
                 (GATE-38-001); without it no requirement set is selected, so \
                 no credit category, no audit state and no graduation \
                 percentage is shown, and no cohort is assumed"
            }
            Self::ProfileGraduationStandard => {
                "which graduation standard the user lawfully selected is a user \
                 input (GATE-38-002); without it the audit block shows the \
                 missing check and the admission year is not read as the \
                 standard"
            }
            Self::ProfileDegreeMode => {
                "whether the user is on a single major, a double major, a \
                 minor, a united or a linked programme is a user input \
                 (GATE-38-003); without it the major average has no scope and \
                 single major is not assumed"
            }
            Self::ProfileAdditionalMajor => {
                "which additional majors or minors the user carries is a user \
                 input (GATE-38-004); section 10's 다전공별 GPA is one figure \
                 per programme, and with no programme named there is no figure \
                 rather than a figure over none"
            }
            Self::ProfileOfficialTranscript => {
                "the current official transcript is a user import (GATE-38-005); \
                 without it there is no attempt set, so the three averages, the \
                 credit totals and the attempt timeline are empty rather than \
                 zero"
            }
            Self::ProfileExchangeOrTransfer => {
                "transferred and exchange credits and their recognition \
                 decisions are user inputs (GATE-38-006); an undecided \
                 recognition reads UNKNOWN on the timeline's 인정 facet and \
                 keeps the affected average out of the numerator rather than \
                 counting it as zero"
            }
            Self::CurrentTermOfferingFacts => {
                "the term's offerings, instructors, capacity, timetable, \
                 syllabus and assessment scheme are official readings the user \
                 supplies (GATE-38-017); this crate fetches none of them, so \
                 every planner candidate is a value a caller read and nothing \
                 is carried over from a term that has passed"
            }
        }
    }
}
