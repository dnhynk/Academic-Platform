//! The one section 38 cell this task leaves open, stated where it bites.
//!
//! `t068` section 5's `P2-U5` entry: *Leaves `GATE-38-017` open per term.*
//!
//! `GATE-38-017` is section 38.2's seventh bullet, and section 38.2's bullets
//! are numbered from eleven because section 38.1's block holds ten lines before
//! them. [`OpenGate::identifier`] is **not** compared against a list written
//! twice: `the_open_gate_is_section_38s_own` reads section 38.2's list out of
//! the design document, takes the bullet at index six, and derives
//! `GATE-38-017` from `six + eleven`. A renumbered section, a reordered bullet
//! and a paraphrased quotation each fail there.
//!
//! `P2-U3` found that eleven of the eighteen `OpenGate::identifier` arms in
//! this workspace were hand-written strings compared only against a hand-written
//! list in the same test, closed it for its own seven, and left the rest as
//! `S-20`. This crate's one arm is derived from the start, so it is not one of
//! the remaining ones.
//!
//! # It stays open every term, and nothing here fills it
//!
//! The cell asks for *해당 학기의 최신 CourseOffering, 교수자, 정원, 시간표,
//! syllabus, 평가 방식* -- the current term's offerings, instructors, capacity,
//! timetable, syllabus and assessment method. Those are facts a user or a
//! connector retrieves, once per term, and they go stale by the next one. So
//! there is no table of them here, no default capacity, no assumed timetable
//! and no cached instructor: what stands while the cell is empty is that
//! `ConfirmationEvidence` cannot be built at all, so no offering is `CONFIRMED`
//! and every standing falls to the forecast or to `UNCERTAIN`.
//!
//! That is also why this crate has no *fill* function and no notion of a gate
//! being closed. Section 38 asks for the reading, not for a decision, and a
//! reading recorded last term is not the reading this term needs.

/// The section 38 cell this crate leaves for the user to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum OpenGate {
    /// `GATE-38-017`: the current term's offerings, instructors, capacity,
    /// timetable, syllabus and assessment method.
    CurrentTermOfferingFacts,
}

impl OpenGate {
    /// Every cell this crate leaves open.
    pub const ALL: [Self; 1] = [Self::CurrentTermOfferingFacts];

    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::CurrentTermOfferingFacts => "GATE-38-017",
        }
    }

    /// The section 38.2 bullet this cell is, verbatim.
    ///
    /// Compared against the design document by
    /// `the_open_gate_is_section_38s_own`, so a paraphrase fails.
    #[must_use]
    pub const fn spec_line(self) -> &'static str {
        match self {
            Self::CurrentTermOfferingFacts => {
                "해당 학기의 최신 CourseOffering, 교수자, 정원, 시간표, syllabus, 평가 방식."
            }
        }
    }

    /// What the user has to supply, and what stands while it is empty.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::CurrentTermOfferingFacts => {
                "the current term's offerings, instructors, capacity, timetable, \
                 syllabus and assessment method are read from the registration \
                 system every term (GATE-38-017); with no reading recorded no \
                 ConfirmationEvidence exists, so nothing is CONFIRMED, a \
                 forecast decides the standing or it is UNCERTAIN, and no \
                 capacity, timetable or instructor is carried over from a term \
                 that has passed"
            }
        }
    }
}
