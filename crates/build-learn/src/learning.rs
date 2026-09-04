//! Section 20.2's `각 학습 항목은 작은 실행 evidence와 다시 project로 돌아가는
//! checkpoint를 갖는다`, and the four-step sequence it gives as its example.
//!
//! > 예: CRDT 개념 읽기 → 두 client merge property를 손으로 설명 → 최소
//! > simulation test → 선택 승인.
//!
//! ## `learning_item_requires_evidence_task_and_checkpoint` is a constructor
//!
//! [`LearningItem::plan`] takes an [`EvidenceTask`] and a [`ReturnCheckpoint`]
//! **by value**. There is no `Default`, no public field, no setter, and no other
//! producer. So a learning item with no evidence task is not a validation
//! failure — it is a value that cannot be built.
//!
//! `crates/build-learn/tests/compile_fail/` holds the compiled half, and
//! `the_only_producer_of_a_learning_item_takes_both` compares the whole set of
//! public functions returning a `LearningItem` against exactly that one, so a
//! second producer added later under any name is caught by a set comparison
//! rather than by a list of names nobody updated.
//!
//! ## The four steps are four types, each consuming the one before
//!
//! ```text
//! ReadingDone --> ExplainedByHand --> SimulationPassed --> SelectionApproved
//! ```
//!
//! Each stage's constructor takes the previous stage **by value**, the way
//! `P2-R5`'s `GeneratedCodeWarrant` takes its three, so `선택 승인` before `최소
//! simulation test` is a program that does not compile. [`ReturnCheckpoint`]
//! holds a [`SelectionApproved`], which is why the sequence is not a list a
//! caller can shuffle: the last stage is the only thing that can be handed over,
//! and it can only exist at the end of the chain.
//!
//! ## `다시 project로 돌아가는`
//!
//! The checkpoint names a step of the plan the user returns to, and
//! [`crate::validate`] refuses a plan in which that step is not an
//! implementation step. That is the half of section 20.2's sentence that a
//! learning item cannot hold on its own — whether the thing it returns to is
//! actually building anything is a property of the plan, not of the item.

use serde::{Deserialize, Serialize};

use academic_domain::EntityId;

use crate::text::{NonEmptyText, PartId};

/// Section 20.2's four checkpoint stages, in the example's own order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckpointStage {
    /// `CRDT 개념 읽기`.
    Read,
    /// `두 client merge property를 손으로 설명`.
    ExplainByHand,
    /// `최소 simulation test`.
    MinimalSimulationOrTest,
    /// `선택 승인`.
    SelectionApproval,
}

/// The four, in the design document's own order.
pub const CHECKPOINT_STAGES: [CheckpointStage; 4] = [
    CheckpointStage::Read,
    CheckpointStage::ExplainByHand,
    CheckpointStage::MinimalSimulationOrTest,
    CheckpointStage::SelectionApproval,
];

impl CheckpointStage {
    /// The design document's own words for this stage.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::Read => "개념 읽기",
            Self::ExplainByHand => "손으로 설명",
            Self::MinimalSimulationOrTest => "최소 simulation test",
            Self::SelectionApproval => "선택 승인",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "READ",
            Self::ExplainByHand => "EXPLAIN_BY_HAND",
            Self::MinimalSimulationOrTest => "MINIMAL_SIMULATION_OR_TEST",
            Self::SelectionApproval => "SELECTION_APPROVAL",
        }
    }
}

/// Stage 1: the reading was done, and it names what was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingDone {
    source: NonEmptyText,
}

impl ReadingDone {
    /// Records the reading.
    #[must_use]
    pub const fn of(source: NonEmptyText) -> Self {
        Self { source }
    }

    /// What was read.
    #[must_use]
    pub const fn source(&self) -> &NonEmptyText {
        &self.source
    }
}

/// Stage 2: the user explained it by hand. Takes stage 1 by value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainedByHand {
    reading: ReadingDone,
    property: NonEmptyText,
}

impl ExplainedByHand {
    /// Records the explanation, consuming the reading.
    #[must_use]
    pub const fn after(reading: ReadingDone, property: NonEmptyText) -> Self {
        Self { reading, property }
    }

    /// The reading it followed.
    #[must_use]
    pub const fn reading(&self) -> &ReadingDone {
        &self.reading
    }

    /// What was explained.
    #[must_use]
    pub const fn property(&self) -> &NonEmptyText {
        &self.property
    }
}

/// Stage 3: the minimal simulation or test passed. Takes stage 2 by value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationPassed {
    explanation: ExplainedByHand,
    artifact: NonEmptyText,
}

impl SimulationPassed {
    /// Records the run, consuming the explanation.
    #[must_use]
    pub const fn after(explanation: ExplainedByHand, artifact: NonEmptyText) -> Self {
        Self {
            explanation,
            artifact,
        }
    }

    /// The explanation it followed.
    #[must_use]
    pub const fn explanation(&self) -> &ExplainedByHand {
        &self.explanation
    }

    /// What was run.
    #[must_use]
    pub const fn artifact(&self) -> &NonEmptyText {
        &self.artifact
    }
}

/// Stage 4: the user approved the selection. Takes stage 3 by value.
///
/// The only thing a [`ReturnCheckpoint`] can be built from, which is why the
/// order of the four is not a list somebody validates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionApproved {
    simulation: SimulationPassed,
    decision: PartId,
    alternative: PartId,
}

impl SelectionApproved {
    /// Records the approval, consuming the simulation.
    #[must_use]
    pub const fn after(
        simulation: SimulationPassed,
        decision: PartId,
        alternative: PartId,
    ) -> Self {
        Self {
            simulation,
            decision,
            alternative,
        }
    }

    /// The simulation it followed.
    #[must_use]
    pub const fn simulation(&self) -> &SimulationPassed {
        &self.simulation
    }

    /// Which decision was approved.
    #[must_use]
    pub const fn decision(&self) -> &PartId {
        &self.decision
    }

    /// Which alternative was approved.
    #[must_use]
    pub const fn alternative(&self) -> &PartId {
        &self.alternative
    }

    /// The four stages this value proves, in order.
    #[must_use]
    pub const fn stages(&self) -> [CheckpointStage; 4] {
        CHECKPOINT_STAGES
    }
}

/// Section 20.2's `작은 실행 evidence`.
///
/// Two required parts: what the user runs, and what its passing would show.
/// Both non-blank, so an evidence task that names nothing runnable is a value
/// that does not exist rather than a plan a reviewer has to catch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTask {
    runs: NonEmptyText,
    shows: NonEmptyText,
}

impl EvidenceTask {
    /// Records one executable evidence task.
    #[must_use]
    pub const fn of(runs: NonEmptyText, shows: NonEmptyText) -> Self {
        Self { runs, shows }
    }

    /// What the user runs.
    #[must_use]
    pub const fn runs(&self) -> &NonEmptyText {
        &self.runs
    }

    /// What its outcome shows.
    #[must_use]
    pub const fn shows(&self) -> &NonEmptyText {
        &self.shows
    }
}

/// Section 20.2's `다시 project로 돌아가는 checkpoint`.
///
/// Holds the fourth stage by value and the plan step the user returns to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReturnCheckpoint {
    approved: SelectionApproved,
    returns_to: PartId,
}

impl ReturnCheckpoint {
    /// Records the return, consuming the approval.
    #[must_use]
    pub const fn of(approved: SelectionApproved, returns_to: PartId) -> Self {
        Self {
            approved,
            returns_to,
        }
    }

    /// The four-stage approval underneath.
    #[must_use]
    pub const fn approved(&self) -> &SelectionApproved {
        &self.approved
    }

    /// The plan step the user returns to.
    #[must_use]
    pub const fn returns_to(&self) -> &PartId {
        &self.returns_to
    }
}

/// One learning item of a build-to-learn plan.
///
/// Private fields, one producer, no `Default`. See the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearningItem {
    id: PartId,
    concept: EntityId,
    evidence_task: EvidenceTask,
    checkpoint: ReturnCheckpoint,
}

impl LearningItem {
    /// Plans one learning item, taking both required parts by value.
    #[must_use]
    pub const fn plan(
        id: PartId,
        concept: EntityId,
        evidence_task: EvidenceTask,
        checkpoint: ReturnCheckpoint,
    ) -> Self {
        Self {
            id,
            concept,
            evidence_task,
            checkpoint,
        }
    }

    /// Its identity within the plan.
    #[must_use]
    pub const fn id(&self) -> &PartId {
        &self.id
    }

    /// The concept it is about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The small executable evidence task.
    #[must_use]
    pub const fn evidence_task(&self) -> &EvidenceTask {
        &self.evidence_task
    }

    /// The return-to-project checkpoint.
    #[must_use]
    pub const fn checkpoint(&self) -> &ReturnCheckpoint {
        &self.checkpoint
    }
}
