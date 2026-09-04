//! The four payloads section 25.2's fourth to seventh lines carry.
//!
//! Each is a record of what some other surface decided, not a second decision.
//! This crate has no edge to the gap engine, the critical-path engine, the
//! question graph or the ingestion pipeline, so nothing here recomputes any of
//! their answers and nothing here can disagree with one. What it adds is the
//! screen those answers appear on, in the order section 25.2 fixes.

use academic_domain::{EntityId, TimestampMillis};

/// Which of the two things section 25.2's fourth line names.
///
/// Both are `사용자가 직접 남긴` — left by the user. Neither is an AI proposal:
/// those live in `P2-X7`'s inbox, on a different screen, and this crate has no
/// edge to `academic-proposal` so it cannot hold one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OpenItemKind {
    /// `열린 질문`.
    OpenQuestion,
    /// `Mark Moment review`.
    MarkMomentReview,
}

impl OpenItemKind {
    /// Exhaustive listing, in the order section 25.2's fourth line names them.
    pub const ALL: [Self; 2] = [Self::OpenQuestion, Self::MarkMomentReview];

    /// The specification's own words for this kind.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::OpenQuestion => "열린 질문",
            Self::MarkMomentReview => "Mark Moment review",
        }
    }
}

/// One thing the user left open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenItem {
    kind: OpenItemKind,
    subject: EntityId,
}

impl OpenItem {
    /// Records one.
    #[must_use]
    pub const fn new(kind: OpenItemKind, subject: EntityId) -> Self {
        Self { kind, subject }
    }

    /// Which of the two kinds.
    #[must_use]
    pub const fn kind(&self) -> OpenItemKind {
        self.kind
    }

    /// The question or the marked moment.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }
}

/// Section 25.2's fifth line: the nearest knowledge need blocking a project.
///
/// Which concept is *nearest* is `P2-N5`'s answer, computed by the gap engine
/// against a real active goal. This crate has no edge to it and computes
/// nothing: it names the concept and the project the answer was about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnowledgeNeed {
    concept: EntityId,
    project: EntityId,
}

impl KnowledgeNeed {
    /// Records one.
    #[must_use]
    pub const fn new(concept: EntityId, project: EntityId) -> Self {
        Self { concept, project }
    }

    /// The concept that is missing.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The project it blocks.
    #[must_use]
    pub const fn project(&self) -> EntityId {
        self.project
    }
}

/// Section 25.2's sixth line, which names two different things.
///
/// **A stale-source warning is not a stale concept.** `P2-N3` fixes that time
/// decay reaches a freshness projection and never a mastery;
/// [`Self::StaleOfficialData`] is a third thing again — the *source* has not
/// been re-read, which is section 25.4's `official source freshness와 마지막
/// sync`. It says nothing about what the user knows and carries no band, and
/// keeping the two apart is why they are separate arms here rather than one
/// "stale" card with a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialCondition {
    /// `deadline이 있는 공식 학사 condition`.
    WithDeadline {
        /// The requirement or condition it is about.
        condition: EntityId,
        /// When it falls due.
        due: TimestampMillis,
    },
    /// `stale official data 경고`.
    StaleOfficialData {
        /// The official source that has not been re-read.
        source: EntityId,
        /// When it was last read.
        last_read: TimestampMillis,
    },
}

impl OfficialCondition {
    /// The deadline, when this arm has one.
    #[must_use]
    pub const fn due(&self) -> Option<TimestampMillis> {
        match self {
            Self::WithDeadline { due, .. } => Some(*due),
            Self::StaleOfficialData { .. } => None,
        }
    }
}

/// Section 25.2's seventh line: the active critical path's next step.
///
/// `사용자 선택` is the whole of that line's weight, and it is not something
/// this crate can verify: the selection is made on `P2-C7`'s surface and this
/// crate has no edge to `academic-critical-path`. What the name says is what
/// the caller is recording. The card holds no score, no ranking and no
/// recommendation, so a step nobody chose has nothing to arrive as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextStep {
    path: EntityId,
    step: EntityId,
}

impl NextStep {
    /// Records the step the user selected on an active path.
    #[must_use]
    pub const fn chosen(path: EntityId, step: EntityId) -> Self {
        Self { path, step }
    }

    /// The active path.
    #[must_use]
    pub const fn path(&self) -> EntityId {
        self.path
    }

    /// The step chosen on it.
    #[must_use]
    pub const fn step(&self) -> EntityId {
        self.step
    }
}
