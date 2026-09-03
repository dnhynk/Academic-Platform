//! Mark Moment: the bare timestamp first, the label later, and no path between
//! them that moves the timestamp.
//!
//! # The rule is structural, not a convention
//!
//! Section 12.2: "Mark Moment는 먼저 한 번의 표시만 저장하고, 세부 label은 수업
//! 후 붙일 수 있다." The failure that rule exists to prevent is a label written
//! after class carrying the instant it was written at.
//!
//! So [`Mark`] has one instant, no label field, and no `&mut self` method at
//! all. A label is a separate [`MarkLabel`] record carrying its own instant and
//! the mark's sequence number, and [`LabelledMark::at`] returns the mark's
//! instant whichever labels exist. `mark_now_label_later` observes that, and
//! `a_label_has_no_path_that_moves_a_mark` compares the whole set of `impl`
//! blocks whose header names `Mark` against a one-entry list, so an accessor
//! nobody predicted fails as an extra key.
//!
//! # Append-only, reusing ADR-003's mechanism rather than a second one
//!
//! Labelling twice appends twice. The current label is the last one applied,
//! which is ADR-003's resolver shape — "Corrections append a new assertion" —
//! rather than a field that is overwritten. The durable half is stronger still:
//! both records are frames in the chain-digested [`crate::journal::ChunkJournal`],
//! so a mark's frame cannot be edited after a label is appended without
//! breaking every digest after it, and `mark_now_label_later` re-reads the file
//! from disk to observe exactly that.

use crate::clock::SessionTick;

/// The five labels section 12.2 draws on the capture surface.
///
/// A closed enum. There is no free-text label, so a label is a classification
/// rather than a note, and no path exists by which user prose enters a capture
/// record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkLabelKind {
    /// 중요.
    Important,
    /// 이해 안 됨.
    NotUnderstood,
    /// 질문.
    Question,
    /// 복습.
    Review,
    /// 강조.
    Emphasis,
}

impl MarkLabelKind {
    /// Every label, in the order section 12.2 draws them.
    pub const ALL: [Self; 5] = [
        Self::Important,
        Self::NotUnderstood,
        Self::Question,
        Self::Review,
        Self::Emphasis,
    ];

    /// The contract spelling, which is also the journal frame's token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Important => "IMPORTANT",
            Self::NotUnderstood => "NOT_UNDERSTOOD",
            Self::Question => "QUESTION",
            Self::Review => "REVIEW",
            Self::Emphasis => "EMPHASIS",
        }
    }

    /// The frame byte.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Important => 1,
            Self::NotUnderstood => 2,
            Self::Question => 3,
            Self::Review => 4,
            Self::Emphasis => 5,
        }
    }

    /// Resolves a label from its frame byte.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.code() == code)
    }
}

/// One Mark Moment: a sequence number and the instant it was made at.
///
/// There is no label field and no method that takes `&mut self`. A value of
/// this type cannot change after it is built, which is the whole of the rule
/// that a later label never shifts the original mark time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    seq: u32,
    at: SessionTick,
}

impl Mark {
    pub(crate) const fn made(seq: u32, at: SessionTick) -> Self {
        Self { seq, at }
    }

    /// Its position among the session's marks, from zero.
    #[must_use]
    pub const fn seq(self) -> u32 {
        self.seq
    }

    /// The instant it was made at. The only instant a mark has.
    #[must_use]
    pub const fn at(self) -> SessionTick {
        self.at
    }
}

/// A label applied to a mark, with the instant the label was applied at.
///
/// `applied_at` is the label's own instant and is never a mark's. It is kept
/// because "when was this labelled" is a real question — a label applied during
/// the lecture and one applied the next morning are different evidence — and
/// because keeping it here is what makes it unnecessary anywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkLabel {
    mark_seq: u32,
    kind: MarkLabelKind,
    applied_at: SessionTick,
}

impl MarkLabel {
    pub(crate) const fn applied(
        mark_seq: u32,
        kind: MarkLabelKind,
        applied_at: SessionTick,
    ) -> Self {
        Self {
            mark_seq,
            kind,
            applied_at,
        }
    }

    /// Which mark it labels.
    #[must_use]
    pub const fn mark_seq(self) -> u32 {
        self.mark_seq
    }

    /// Which label.
    #[must_use]
    pub const fn kind(self) -> MarkLabelKind {
        self.kind
    }

    /// When the label was applied. Not the mark's instant.
    #[must_use]
    pub const fn applied_at(self) -> SessionTick {
        self.applied_at
    }
}

/// A mark read together with the labels applied to it.
///
/// [`LabelledMark::at`] returns the mark's instant. There is no other instant
/// on this type and no accessor that yields a label's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelledMark {
    mark: Mark,
    label: Option<MarkLabel>,
}

impl LabelledMark {
    /// The mark itself.
    #[must_use]
    pub const fn mark(self) -> Mark {
        self.mark
    }

    /// The instant the mark was made at — never a label's.
    #[must_use]
    pub const fn at(self) -> SessionTick {
        self.mark.at()
    }

    /// The current label, which is the last one appended, or `None`.
    #[must_use]
    pub const fn label(self) -> Option<MarkLabelKind> {
        match self.label {
            Some(label) => Some(label.kind()),
            None => None,
        }
    }
}

/// Why a label was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MarkFault {
    /// No mark carries that sequence number.
    #[error("no mark with sequence {seq}")]
    UnknownMark {
        /// The sequence that was offered.
        seq: u32,
    },
}

/// Every mark and every label a session has appended.
///
/// Two append-only vectors and no removal. A correction is another
/// [`MarkLabel`], and the resolver reads the last one.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MarkLedger {
    marks: Vec<Mark>,
    labels: Vec<MarkLabel>,
}

impl MarkLedger {
    /// An empty ledger.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marks: Vec::new(),
            labels: Vec::new(),
        }
    }

    /// Appends a mark at `at` and returns it.
    pub(crate) fn append_mark(&mut self, at: SessionTick) -> Mark {
        let seq = u32::try_from(self.marks.len()).unwrap_or(u32::MAX);
        let mark = Mark::made(seq, at);
        self.marks.push(mark);
        mark
    }

    /// Appends a label against an existing mark.
    ///
    /// It takes the mark's sequence number rather than a `&mut Mark`, so there
    /// is no borrow of a mark through which one could be written.
    pub(crate) fn append_label(
        &mut self,
        mark_seq: u32,
        kind: MarkLabelKind,
        applied_at: SessionTick,
    ) -> Result<MarkLabel, MarkFault> {
        if !self.marks.iter().any(|mark| mark.seq() == mark_seq) {
            return Err(MarkFault::UnknownMark { seq: mark_seq });
        }
        let label = MarkLabel::applied(mark_seq, kind, applied_at);
        self.labels.push(label);
        Ok(label)
    }

    /// Every mark, in the order they were made.
    #[must_use]
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Every label, in the order they were applied.
    #[must_use]
    pub fn labels(&self) -> &[MarkLabel] {
        &self.labels
    }

    /// One mark with its current label resolved.
    #[must_use]
    pub fn resolve(&self, mark_seq: u32) -> Option<LabelledMark> {
        let mark = *self.marks.iter().find(|mark| mark.seq() == mark_seq)?;
        let label = self
            .labels
            .iter()
            .rev()
            .find(|label| label.mark_seq() == mark_seq)
            .copied();
        Some(LabelledMark { mark, label })
    }
}
