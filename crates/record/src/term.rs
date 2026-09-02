//! The ordered term identity every effective-dated policy row is compared on.
//!
//! An effective date in this domain is not a calendar instant. The official
//! notice this crate encodes says a ceiling applies "2015학년도 1학기 이수
//! 교과목부터" — from courses *taken in* 2015 spring onward — so the thing that
//! orders a policy row against an attempt is the academic term, not a
//! timestamp. Comparing on a date would need a term-to-date table nobody has
//! confirmed, and would put a summer-session attempt on the wrong side of a
//! boundary that was never drawn there.
//!
//! [`TermKey`] is therefore the whole ordering: an academic year and one of
//! four sessions. Its spelling is the specification's own (`2026_FALL`), and it
//! is identifier-shaped, so it travels through a deterministic engine's frozen
//! inputs as a `ref:` value with no second encoding.

use core::{cmp::Ordering, fmt};

use crate::RecordError;

/// One of the four sessions an academic year is divided into.
///
/// Ordered as the year runs, which is what makes [`TermKey`] comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Semester {
    /// 1학기.
    Spring,
    /// 여름 계절학기.
    Summer,
    /// 2학기.
    Fall,
    /// 겨울 계절학기.
    Winter,
}

impl Semester {
    /// Every session, in the order the academic year runs.
    pub const ALL: [Self; 4] = [Self::Spring, Self::Summer, Self::Fall, Self::Winter];

    /// Returns the contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spring => "SPRING",
            Self::Summer => "SUMMER",
            Self::Fall => "FALL",
            Self::Winter => "WINTER",
        }
    }

    /// Resolves a session from its contract spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|semester| semester.as_str() == text)
    }
}

/// An academic year and session, ordered.
///
/// The year is the 학년도 the term belongs to, not the calendar year a winter
/// session happens to fall in; the two differ for a winter term and the
/// official notices are written in 학년도.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TermKey {
    year: u16,
    semester: Semester,
}

impl TermKey {
    /// Builds a term key, refusing a year outside the representable range.
    ///
    /// The bound is deliberately wide and deliberately finite: a four-digit
    /// year keeps the canonical spelling fixed-width, which is what lets the
    /// spelling be compared as text anywhere the ordering is not needed.
    pub fn new(year: u16, semester: Semester) -> Result<Self, RecordError> {
        if !(1000..=9999).contains(&year) {
            return Err(RecordError::TermYearOutOfRange(year));
        }
        Ok(Self { year, semester })
    }

    /// Returns the 학년도.
    #[must_use]
    pub const fn year(self) -> u16 {
        self.year
    }

    /// Returns the session.
    #[must_use]
    pub const fn semester(self) -> Semester {
        self.semester
    }

    /// Returns the canonical, identifier-shaped spelling, e.g. `2026_FALL`.
    #[must_use]
    pub fn canonical_text(self) -> String {
        format!("{}_{}", self.year, self.semester.as_str())
    }

    /// Parses a term as an official transcript spells it.
    ///
    /// `P2-U7` fixes no term spelling: a `TranscriptRow`'s term is whatever the
    /// document wrote, and the reconciler compares two readings of it without
    /// interpreting either. `P2-U4` needs an *ordered* term, because that is
    /// what an effective-dated policy row is compared against, so the crossing
    /// needs a declared mapping.
    ///
    /// Two spellings are admitted and the rest are refused:
    ///
    /// | spelling | term |
    /// |---|---|
    /// | `2024_FALL` | the canonical form, section 10's own (`term: 2026_FALL`) |
    /// | `2024-1`, `2024-2` | 1학기 and 2학기 |
    ///
    /// **A 계절학기 is refused, not guessed.** No source in this repository
    /// states how a summer or winter term is written on a transcript, and a
    /// term placed in the wrong session by a guess would move an attempt to the
    /// wrong side of an effective date. A refusal names the field; section 38
    /// says an unconfirmed official fact stays unknown.
    pub fn parse_transcript_term(text: &str) -> Result<Self, RecordError> {
        if let Some((year, ordinal)) = text.split_once('-') {
            let year: u16 = year
                .parse()
                .map_err(|_| RecordError::MalformedTerm(text.to_owned()))?;
            let semester = match ordinal {
                "1" => Semester::Spring,
                "2" => Semester::Fall,
                _ => return Err(RecordError::UnconfirmedTermSpelling(text.to_owned())),
            };
            return Self::new(year, semester);
        }
        Self::parse(text)
    }

    /// Parses the canonical spelling.
    pub fn parse(text: &str) -> Result<Self, RecordError> {
        let (year, semester) = text
            .split_once('_')
            .ok_or_else(|| RecordError::MalformedTerm(text.to_owned()))?;
        let year: u16 = year
            .parse()
            .map_err(|_| RecordError::MalformedTerm(text.to_owned()))?;
        let semester =
            Semester::parse(semester).ok_or_else(|| RecordError::MalformedTerm(text.to_owned()))?;
        Self::new(year, semester)
    }
}

impl PartialOrd for TermKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TermKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.year
            .cmp(&other.year)
            .then(self.semester.cmp(&other.semester))
    }
}

impl fmt::Display for TermKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical_text())
    }
}
