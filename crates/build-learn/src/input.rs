//! Section 20's six input kinds, and the one thing normalisation is forbidden
//! to produce.
//!
//! ## The six
//!
//! > 사용자는 자연어 기능, ProjectGoal, 초기 spec, 빈 repo, 진행 중 repo,
//! > architecture idea 중 하나를 입력한다.
//!
//! [`InputKind`] is those six and no seventh, and [`INPUT_KINDS`] is them in the
//! sentence's own order. `six_input_kinds_normalize` reads the sentence back out
//! of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares it with
//! [`INPUT_KINDS`] in both directions, so six is a measurement of the design
//! document rather than a number written here.
//!
//! ## Normalisation produces an intent, and an intent is not a technology list
//!
//! > 시스템은 이를 바로 기술 목록으로 바꾸지 않고 성공 조건과 선택 지점을
//! > 추출한다.
//!
//! [`normalize`] returns a [`NormalizedIntent`], which holds the capability the
//! input is about and the source kind it arrived as. It holds no concept, no
//! requirement and no named technology, because the only value in this crate
//! that may carry a named technology is
//! [`crate::technology::TechnologySlate`], whose one constructor takes a
//! [`crate::goal::ProjectGoal`] — and a `ProjectGoal` cannot be built without
//! its success criteria. See [`crate::technology`].

use serde::{Deserialize, Serialize};

use crate::{BuildLearnError, text::NonEmptyText};

/// Section 20.1's six input kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InputKind {
    /// `자연어 기능`: a sentence about a feature the user wants.
    NaturalLanguageFeature,
    /// `ProjectGoal`: a goal document the user already wrote.
    ProjectGoalDocument,
    /// `초기 spec`: a specification written before any code.
    InitialSpec,
    /// `빈 repo`: a repository snapshot with nothing analysed in it yet.
    EmptyRepository,
    /// `진행 중 repo`: a snapshot `P2-R4` has already classified.
    InProgressRepository,
    /// `architecture idea`: a sketch of a structure, with its choice points.
    ArchitectureIdea,
}

/// The six, in the design document's own order.
pub const INPUT_KINDS: [InputKind; 6] = [
    InputKind::NaturalLanguageFeature,
    InputKind::ProjectGoalDocument,
    InputKind::InitialSpec,
    InputKind::EmptyRepository,
    InputKind::InProgressRepository,
    InputKind::ArchitectureIdea,
];

impl InputKind {
    /// The words the design document uses, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::NaturalLanguageFeature => "자연어 기능",
            Self::ProjectGoalDocument => "ProjectGoal",
            Self::InitialSpec => "초기 spec",
            Self::EmptyRepository => "빈 repo",
            Self::InProgressRepository => "진행 중 repo",
            Self::ArchitectureIdea => "architecture idea",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NaturalLanguageFeature => "NATURAL_LANGUAGE_FEATURE",
            Self::ProjectGoalDocument => "PROJECT_GOAL_DOCUMENT",
            Self::InitialSpec => "INITIAL_SPEC",
            Self::EmptyRepository => "EMPTY_REPOSITORY",
            Self::InProgressRepository => "IN_PROGRESS_REPOSITORY",
            Self::ArchitectureIdea => "ARCHITECTURE_IDEA",
        }
    }
}

/// What one input offered, before anything was extracted from it.
///
/// One variant per [`InputKind`], each carrying what that kind actually has. A
/// repository input carries the snapshot identity the analysis was taken over
/// rather than any analysed byte: nothing in this crate opens a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalInput {
    /// A sentence.
    NaturalLanguageFeature {
        /// What the user said they want to build.
        sentence: NonEmptyText,
    },
    /// A goal document the user already wrote.
    ProjectGoalDocument {
        /// Its text.
        text: NonEmptyText,
    },
    /// A specification written before there is code to read.
    InitialSpec {
        /// The specification's title.
        title: NonEmptyText,
        /// Its statements, in document order.
        statements: Vec<NonEmptyText>,
    },
    /// A snapshot with nothing analysed in it.
    EmptyRepository {
        /// `P2-R1`'s snapshot identity.
        snapshot_id: NonEmptyText,
        /// What the user says the empty repository is going to become.
        intended: NonEmptyText,
    },
    /// A snapshot `P2-R4` has classified.
    InProgressRepository {
        /// `P2-R1`'s snapshot identity.
        snapshot_id: NonEmptyText,
        /// What the user wants to add to what is already there.
        wanted: NonEmptyText,
    },
    /// A structure sketch, with the points it has not decided.
    ArchitectureIdea {
        /// The structure the user has in mind.
        sketch: NonEmptyText,
    },
}

impl GoalInput {
    /// Which of the six this is.
    #[must_use]
    pub const fn kind(&self) -> InputKind {
        match self {
            Self::NaturalLanguageFeature { .. } => InputKind::NaturalLanguageFeature,
            Self::ProjectGoalDocument { .. } => InputKind::ProjectGoalDocument,
            Self::InitialSpec { .. } => InputKind::InitialSpec,
            Self::EmptyRepository { .. } => InputKind::EmptyRepository,
            Self::InProgressRepository { .. } => InputKind::InProgressRepository,
            Self::ArchitectureIdea { .. } => InputKind::ArchitectureIdea,
        }
    }

    /// The snapshot this input was taken over, when it is a repository.
    #[must_use]
    pub const fn snapshot_id(&self) -> Option<&NonEmptyText> {
        match self {
            Self::EmptyRepository { snapshot_id, .. }
            | Self::InProgressRepository { snapshot_id, .. } => Some(snapshot_id),
            Self::NaturalLanguageFeature { .. }
            | Self::ProjectGoalDocument { .. }
            | Self::InitialSpec { .. }
            | Self::ArchitectureIdea { .. } => None,
        }
    }
}

/// What section 20.1's normalisation produces: a capability and its source.
///
/// Private fields, one producer, no `Default`. It carries the capability the
/// input is about and the kind it arrived as, and it is what
/// [`crate::goal::ProjectGoal::state`] takes as its first argument — so a goal
/// always knows which of the six it came from, which is REQ-20-001's `source
/// type retained`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedIntent {
    source: InputKind,
    capability: NonEmptyText,
    snapshot_id: Option<NonEmptyText>,
}

impl NormalizedIntent {
    /// Which of the six kinds this came from.
    #[must_use]
    pub const fn source(&self) -> InputKind {
        self.source
    }

    /// The capability the input is about.
    #[must_use]
    pub const fn capability(&self) -> &NonEmptyText {
        &self.capability
    }

    /// The snapshot the input was taken over, for the two repository kinds.
    #[must_use]
    pub const fn snapshot_id(&self) -> Option<&NonEmptyText> {
        self.snapshot_id.as_ref()
    }
}

/// Section 20.1's normalisation, over any of the six kinds.
///
/// The capability is the input's own words about what is wanted, chosen per
/// kind: a specification's title rather than its first statement, an empty
/// repository's `intended` rather than its identity. It is deliberately the
/// user's text and not a rewriting of it, because what comes next is extracting
/// criteria and choices from it, and that is [`crate::goal::ProjectGoal::state`]'s
/// job.
///
/// # Errors
///
/// [`BuildLearnError::SpecificationHasNoStatement`] when an
/// [`GoalInput::InitialSpec`] carries no statement. Every other kind's fields
/// are [`NonEmptyText`], so there is no empty case left for them to be in.
pub fn normalize(input: &GoalInput) -> Result<NormalizedIntent, BuildLearnError> {
    let capability = match input {
        GoalInput::NaturalLanguageFeature { sentence } => sentence.clone(),
        GoalInput::ProjectGoalDocument { text } => text.clone(),
        GoalInput::InitialSpec { title, statements } => {
            if statements.is_empty() {
                return Err(BuildLearnError::SpecificationHasNoStatement);
            }
            title.clone()
        }
        GoalInput::EmptyRepository { intended, .. } => intended.clone(),
        GoalInput::InProgressRepository { wanted, .. } => wanted.clone(),
        GoalInput::ArchitectureIdea { sketch } => sketch.clone(),
    };
    Ok(NormalizedIntent {
        source: input.kind(),
        capability,
        snapshot_id: input.snapshot_id().cloned(),
    })
}
