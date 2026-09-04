//! Section 12.7's seven places, and the reference that binds a claim to one.
//!
//! ## Seven is a measurement, not a number this file chose
//!
//! > syllabus, 다음 title/slide, 교재 chapter, LMS 자료, 과제, 공지,
//! > 직전 강의 말미에서 `ExpectedConceptClaim`을 추출한다.
//!
//! [`EXPECTED_CONCEPT_SOURCES`] holds those items in the sentence's own order
//! and `expected_concept_source_matrix` reads the sentence back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits it on the
//! document's own commas and compares the two in both directions. It then
//! removes every matched cell from the sentence and requires what is left to be
//! separators, so an eighth place leaves text behind and fails rather than being
//! folded into the nearest arm. `P2-X2`'s
//! `permission_status_is_exactly_four_values` is the same reading.
//!
//! **`다음 title/slide` is one cell and not two.** The sentence separates its
//! places with `, ` and this one holds a `/` inside a single comma-delimited
//! item, exactly as `synonym/granularity` does in section 15.2's table. Reading
//! the slash as a separator would make the count eight while the document is
//! punctuated for seven, and the removal pass above is what would then fail.
//! [the next-lecture contract](../../../docs/contracts/next-lecture-preparation.md)
//! records the reading.
//!
//! ## The role vocabulary is not `P2-G5`'s document vocabulary
//!
//! [`ExpectedConceptSource`] answers *which of section 12.7's places is this*.
//! `SourceKind` answers *what kind of document arrived*. They are different
//! questions with different cardinalities — six kinds against seven places — and
//! this crate maps neither onto the other. What it does instead is bind them at
//! the one point where a mismatch would matter: [`MaterialReference::of`] takes
//! the `SourceId` of an ingested document, and
//! [`crate::claim::ExpectedConceptClaim::extract`] refuses a claim whose
//! material is not among the spans the model actually cited. So a claim labelled
//! `SYLLABUS` that quotes a README is a value that cannot be built, without this
//! crate ever deciding which `SourceKind` a syllabus has.

use academic_ingestion::dating::Date;
use academic_lecture_document::NodeId;
use academic_untrusted_content::SourceId;
use serde::{Deserialize, Serialize};

use crate::NextLectureError;

/// One of section 12.7's seven places an expected concept is extracted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExpectedConceptSource {
    /// `syllabus`.
    Syllabus,
    /// `다음 title/slide`.
    NextTitleOrSlide,
    /// `교재 chapter`.
    TextbookChapter,
    /// `LMS 자료`.
    LmsMaterial,
    /// `과제`.
    Assignment,
    /// `공지`.
    Notice,
    /// `직전 강의 말미`.
    PriorLectureEnding,
}

/// The seven, in section 12.7's own order.
///
/// The same array under a module-level name, because `P2-N5` and `P2-P3` each
/// read one and `every_all_array_is_the_enum_it_names` walks the other. There
/// is one literal list and it is [`ExpectedConceptSource::ALL`].
pub const EXPECTED_CONCEPT_SOURCES: [ExpectedConceptSource; 7] = ExpectedConceptSource::ALL;

impl ExpectedConceptSource {
    /// Exhaustive order, in section 12.7's own sentence order.
    pub const ALL: [Self; 7] = [
        Self::Syllabus,
        Self::NextTitleOrSlide,
        Self::TextbookChapter,
        Self::LmsMaterial,
        Self::Assignment,
        Self::Notice,
        Self::PriorLectureEnding,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syllabus => "SYLLABUS",
            Self::NextTitleOrSlide => "NEXT_TITLE_OR_SLIDE",
            Self::TextbookChapter => "TEXTBOOK_CHAPTER",
            Self::LmsMaterial => "LMS_MATERIAL",
            Self::Assignment => "ASSIGNMENT",
            Self::Notice => "NOTICE",
            Self::PriorLectureEnding => "PRIOR_LECTURE_ENDING",
        }
    }

    /// The cell section 12.7 writes for this place, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::Syllabus => "syllabus",
            Self::NextTitleOrSlide => "다음 title/slide",
            Self::TextbookChapter => "교재 chapter",
            Self::LmsMaterial => "LMS 자료",
            Self::Assignment => "과제",
            Self::Notice => "공지",
            Self::PriorLectureEnding => "직전 강의 말미",
        }
    }

    /// Whether this place is a lecture this system already recorded.
    ///
    /// Exactly one of the seven is, and it is the one whose material cites a
    /// `P2-L4` node. A total match rather than an equality test, so a seventh
    /// place added later has to answer the question.
    #[must_use]
    pub const fn is_recorded_by_this_system(self) -> bool {
        match self {
            Self::PriorLectureEnding => true,
            Self::Syllabus
            | Self::NextTitleOrSlide
            | Self::TextbookChapter
            | Self::LmsMaterial
            | Self::Assignment
            | Self::Notice => false,
        }
    }
}

/// Which material one claim was extracted from, and when that material is from.
///
/// Section 27.1's confirmation condition for this row is
/// `자료 날짜·state·edge 불확실성 노출`, so the date is a parameter rather than a
/// field a caller may leave unset, and it is `P2-U6`'s validated calendar
/// [`Date`] rather than a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialReference {
    source: ExpectedConceptSource,
    document: SourceId,
    published: Date,
    lecture_node: Option<NodeId>,
}

impl MaterialReference {
    /// Records one material.
    ///
    /// # Errors
    ///
    /// [`NextLectureError::PriorLectureEndingNeedsItsDocumentNode`] when the
    /// prior lecture's ending cites no `P2-L4` node, and
    /// [`NextLectureError::OnlyThePriorLectureEndingCitesADocumentNode`] when
    /// any other place cites one. The seventh place is the only one this system
    /// recorded itself; the other six arrived as bytes from outside, and a
    /// document node on one of those would claim a preservation that never
    /// happened.
    pub fn of(
        source: ExpectedConceptSource,
        document: SourceId,
        published: Date,
        lecture_node: Option<NodeId>,
    ) -> Result<Self, NextLectureError> {
        match (source.is_recorded_by_this_system(), &lecture_node) {
            (true, None) => {
                return Err(NextLectureError::PriorLectureEndingNeedsItsDocumentNode);
            }
            (false, Some(_)) => {
                return Err(
                    NextLectureError::OnlyThePriorLectureEndingCitesADocumentNode { place: source },
                );
            }
            (true, Some(_)) | (false, None) => {}
        }
        Ok(Self {
            source,
            document,
            published,
            lecture_node,
        })
    }

    /// Which of section 12.7's seven places.
    #[must_use]
    pub const fn source(&self) -> ExpectedConceptSource {
        self.source
    }

    /// The `P2-G5` identity of the document the claim's spans point into.
    #[must_use]
    pub const fn document(&self) -> &SourceId {
        &self.document
    }

    /// Section 27.1's `자료 날짜`.
    #[must_use]
    pub const fn published(&self) -> Date {
        self.published
    }

    /// The `P2-L4` node, for the one place this system recorded.
    #[must_use]
    pub const fn lecture_node(&self) -> Option<&NodeId> {
        self.lecture_node.as_ref()
    }
}
