//! Section 15.3's explanation contract, and the validator that refuses advice
//! too broad to act on.
//!
//! ## Eight fields, and the count is a measurement
//!
//! > 모든 Gap 제안은 `무엇`, `왜 막는가`, `근거`, `confidence`, `현재 상태`,
//! > `최소 보강`, `대체 경로`, `연결된 강의/프로젝트`를 포함한다.
//!
//! [`EXPLANATION_FIELDS`] holds those eight tokens in the sentence's own order
//! and `eight_field_explanation_is_complete` reads the sentence back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares the two
//! in both directions. [`GapExplanation`] has one field per token, all private,
//! and one constructor that takes all eight, so an explanation missing one is a
//! value that cannot be built rather than a report with a blank cell.
//!
//! ## The validator holds no words
//!
//! Section 15.3's second sentence is `“데이터베이스를 더 공부하세요”는 너무 넓어
//! 유효한 Gap 설명이 아니다`. A validator that refused that sentence by matching
//! its words would pass the next paraphrase, so **this crate contains no list of
//! broad phrases and no text matching of any kind**. Every one of
//! [`SpecificityDefect`]'s reasons is a structural fact:
//!
//! * `Database` is a `FIELD`, and `P2-C3` says a field `carries no independent
//!   prerequisite of its own` — so the subject tier refuses it;
//! * `더 공부하세요` cites no evidence item, states no duration and names no
//!   source to study, so `근거` and `최소 보강` are empty;
//! * it names no alternative and no lecture or project.
//!
//! `generic_advice_fails_validation` therefore drives a *fluent, plausible,
//! entirely reasonable-sounding* recommendation that uses none of section 15.3's
//! words, and observes the same defects. `the_gap_crate_holds_no_phrase_list`
//! observes the other half: the crate's product sources contain no string
//! literal long enough to be a phrase to match against, outside the design
//! document's own quoted cells.
//!
//! ## Every field a person could leave vague is typed
//!
//! `무엇` is an identity and a tier, not a noun phrase. `왜 막는가` is a
//! [`crate::path::BlockingPath`] with a strength on every hop. `근거` is a list
//! of evidence identities. `현재 상태` is the four-dimension overlay. `최소 보강`
//! states a positive number of minutes, cites at least one source, and names the
//! activity shape section 15.2's own `예시 대응` column gives this kind. `대체
//! 경로` is either routes or a closed reason. Only the two human sentences —
//! the remediation's own description and nothing else — are free text, and
//! neither is what the validator reads.

use academic_domain::{ConfidencePermille, EntityId, EvidenceId, entity_registry::EntityKind};
use serde::{Deserialize, Serialize};

use crate::{GapError, kind::GapKind, node::gap_bearing, path::BlockingPath, state::StateSnapshot};

/// Section 15.3's eight required fields, in the sentence's own order.
pub const EXPLANATION_FIELDS: [&str; 8] = [
    "무엇",
    "왜 막는가",
    "근거",
    "confidence",
    "현재 상태",
    "최소 보강",
    "대체 경로",
    "연결된 강의/프로젝트",
];

/// The activity shape section 15.2's `예시 대응` column gives each kind.
///
/// Five kinds and five shapes. [`RemediationActivity::for_kind`] is total with
/// no wildcard arm, so a remediation whose shape does not match its kind is a
/// defect the validator names rather than a plausible-looking suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RemediationActivity {
    /// `기초 설명·문제·실험`.
    FoundationalExplanationProblemOrExperiment,
    /// `짧은 retrieval/refresher`.
    ShortRetrievalOrRefresher,
    /// `사용자 확인 또는 diagnostic`.
    UserConfirmationOrDiagnostic,
    /// `merge/sense correction`.
    MergeOrSenseCorrection,
    /// `선택지와 조건 명확화`.
    OptionsAndConditionsClarified,
}

impl RemediationActivity {
    /// The shape section 15.2's table names for `kind`.
    #[must_use]
    pub const fn for_kind(kind: GapKind) -> Self {
        match kind {
            GapKind::MasteryGap => Self::FoundationalExplanationProblemOrExperiment,
            GapKind::FreshnessGap => Self::ShortRetrievalOrRefresher,
            GapKind::EvidenceGap => Self::UserConfirmationOrDiagnostic,
            GapKind::OntologyGap => Self::MergeOrSenseCorrection,
            GapKind::ContextGap => Self::OptionsAndConditionsClarified,
        }
    }

    /// The table cell this shape is, verbatim.
    #[must_use]
    pub const fn cell(self) -> &'static str {
        match self {
            Self::FoundationalExplanationProblemOrExperiment => "기초 설명·문제·실험",
            Self::ShortRetrievalOrRefresher => "짧은 retrieval/refresher",
            Self::UserConfirmationOrDiagnostic => "사용자 확인 또는 diagnostic",
            Self::MergeOrSenseCorrection => "merge/sense correction",
            Self::OptionsAndConditionsClarified => "선택지와 조건 명확화",
        }
    }
}

/// Section 15.3's `최소 보강`.
///
/// Section 36.4's own remediation is `25분짜리 최소 보강` with a lecture source
/// and a small experiment beside it, and the sentence after it is `사용자는 전체
/// storage course를 미리 공부하지 않고 이 경로만 수행한다`. The difference
/// between the two is not a word: it is that one states how long it takes and
/// what to read, and the other states neither.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimumRemediation {
    minutes: u16,
    activity: RemediationActivity,
    description: String,
    sources: Vec<EvidenceId>,
}

impl MinimumRemediation {
    /// Records one bounded, cited remediation.
    #[must_use]
    pub fn of(
        minutes: u16,
        activity: RemediationActivity,
        description: &str,
        sources: Vec<EvidenceId>,
    ) -> Self {
        Self {
            minutes,
            activity,
            description: description.to_owned(),
            sources,
        }
    }

    /// How long it takes. Zero means unbounded, which the validator refuses.
    #[must_use]
    pub const fn minutes(&self) -> u16 {
        self.minutes
    }

    /// Which of section 15.2's five response shapes it is.
    #[must_use]
    pub const fn activity(&self) -> RemediationActivity {
        self.activity
    }

    /// The human sentence. Not read by the validator.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// What to read, run or answer.
    #[must_use]
    pub fn sources(&self) -> &[EvidenceId] {
        &self.sources
    }
}

/// Why no alternative route exists, when none does.
///
/// A closed set of graph facts, so `대체 경로` is never an empty cell and never
/// a sentence somebody wrote to fill it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoAlternativeReason {
    /// The only admitted edge into the goal from here is `HARD`, so section
    /// 7.2's `없으면 목표 수행이 신뢰성 있게 막히는` leaves no other route.
    SoleHardPrerequisite,
    /// No other traversable edge reaches this node's advanced end at all.
    NoOtherAdmittedEdge,
}

/// Section 15.3's `대체 경로`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "alternative", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlternativePath {
    /// Routes that reach the same goal without this node.
    Routes {
        /// One entry per route, each a concept sequence.
        routes: Vec<Vec<EntityId>>,
    },
    /// There is none, and the graph says why.
    None {
        /// Which graph fact rules one out.
        reason: NoAlternativeReason,
    },
}

/// Section 15.3's `연결된 강의/프로젝트`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LinkedContext {
    /// Lectures this concept is taught in.
    pub lectures: Vec<EntityId>,
    /// Project snapshots it is applied in.
    pub projects: Vec<EntityId>,
}

impl LinkedContext {
    /// Whether anything at all is linked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lectures.is_empty() && self.projects.is_empty()
    }
}

/// One structural reason an explanation is too broad to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpecificityDefect {
    /// `무엇` names a tier that carries no independent prerequisite of its own.
    SubjectCarriesNoPrerequisite,
    /// `왜 막는가` ends somewhere other than the subject, or has no hop at all.
    BlockingPathDoesNotReachSubject,
    /// `근거` cites no evidence item.
    NoEvidenceCited,
    /// `최소 보강` states no duration.
    RemediationUnbounded,
    /// `최소 보강` names nothing to read, run or answer.
    RemediationUncited,
    /// `최소 보강`'s shape is not the one section 15.2 gives this kind.
    RemediationDoesNotMatchKind,
    /// `대체 경로` offers an empty route list rather than a reason.
    AlternativeIsEmpty,
    /// `연결된 강의/프로젝트` names neither.
    NoLinkedContext,
}

impl SpecificityDefect {
    /// Exhaustive order.
    pub const ALL: [Self; 8] = [
        Self::SubjectCarriesNoPrerequisite,
        Self::BlockingPathDoesNotReachSubject,
        Self::NoEvidenceCited,
        Self::RemediationUnbounded,
        Self::RemediationUncited,
        Self::RemediationDoesNotMatchKind,
        Self::AlternativeIsEmpty,
        Self::NoLinkedContext,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SubjectCarriesNoPrerequisite => "SUBJECT_CARRIES_NO_PREREQUISITE",
            Self::BlockingPathDoesNotReachSubject => "BLOCKING_PATH_DOES_NOT_REACH_SUBJECT",
            Self::NoEvidenceCited => "NO_EVIDENCE_CITED",
            Self::RemediationUnbounded => "REMEDIATION_UNBOUNDED",
            Self::RemediationUncited => "REMEDIATION_UNCITED",
            Self::RemediationDoesNotMatchKind => "REMEDIATION_DOES_NOT_MATCH_KIND",
            Self::AlternativeIsEmpty => "ALTERNATIVE_IS_EMPTY",
            Self::NoLinkedContext => "NO_LINKED_CONTEXT",
        }
    }

    /// Which of section 15.3's eight fields the defect is about.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::SubjectCarriesNoPrerequisite => EXPLANATION_FIELDS[0],
            Self::BlockingPathDoesNotReachSubject => EXPLANATION_FIELDS[1],
            Self::NoEvidenceCited => EXPLANATION_FIELDS[2],
            Self::RemediationUnbounded
            | Self::RemediationUncited
            | Self::RemediationDoesNotMatchKind => EXPLANATION_FIELDS[5],
            Self::AlternativeIsEmpty => EXPLANATION_FIELDS[6],
            Self::NoLinkedContext => EXPLANATION_FIELDS[7],
        }
    }
}

/// The eight fields section 15.3 requires, and nothing else.
///
/// Private fields, one constructor, and the constructor runs
/// [`GapExplanation::defects`] before it returns, so a broad recommendation has
/// no value of this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapExplanation {
    kind: GapKind,
    subject: EntityId,
    subject_kind: EntityKind,
    blocks: BlockingPath,
    evidence: Vec<EvidenceId>,
    confidence: ConfidencePermille,
    current_state: StateSnapshot,
    remediation: MinimumRemediation,
    alternative: AlternativePath,
    linked: LinkedContext,
}

/// Everything one explanation needs, so the constructor takes one argument
/// rather than ten positional ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationParts {
    /// Which of section 15.2's five kinds.
    pub kind: GapKind,
    /// `무엇` — the concept.
    pub subject: EntityId,
    /// `무엇` — its tier.
    pub subject_kind: EntityKind,
    /// `왜 막는가`.
    pub blocks: BlockingPath,
    /// `근거`.
    pub evidence: Vec<EvidenceId>,
    /// `confidence`.
    pub confidence: ConfidencePermille,
    /// `현재 상태`.
    pub current_state: StateSnapshot,
    /// `최소 보강`.
    pub remediation: MinimumRemediation,
    /// `대체 경로`.
    pub alternative: AlternativePath,
    /// `연결된 강의/프로젝트`.
    pub linked: LinkedContext,
}

impl GapExplanation {
    /// Builds an explanation, or refuses it.
    ///
    /// # Errors
    ///
    /// [`GapError::NotSpecific`] carrying every defect, not the first.
    pub fn of(parts: ExplanationParts) -> Result<Self, GapError> {
        let value = Self {
            kind: parts.kind,
            subject: parts.subject,
            subject_kind: parts.subject_kind,
            blocks: parts.blocks,
            evidence: parts.evidence,
            confidence: parts.confidence,
            current_state: parts.current_state,
            remediation: parts.remediation,
            alternative: parts.alternative,
            linked: parts.linked,
        };
        let defects = value.defects();
        if defects.is_empty() {
            Ok(value)
        } else {
            Err(GapError::NotSpecific(defects))
        }
    }

    /// Every structural reason this explanation is too broad, in
    /// [`SpecificityDefect::ALL`] order.
    ///
    /// All eight rules are evaluated; the result is the whole list rather than
    /// the first entry, which is `P2-N2`'s `blocking_reasons` shape.
    #[must_use]
    pub fn defects(&self) -> Vec<SpecificityDefect> {
        let mut found = Vec::new();
        if !gap_bearing(self.subject_kind) {
            found.push(SpecificityDefect::SubjectCarriesNoPrerequisite);
        }
        if self.blocks.tip() != self.subject || self.blocks.steps().is_empty() {
            found.push(SpecificityDefect::BlockingPathDoesNotReachSubject);
        }
        if self.evidence.is_empty() {
            found.push(SpecificityDefect::NoEvidenceCited);
        }
        if self.remediation.minutes() == 0 {
            found.push(SpecificityDefect::RemediationUnbounded);
        }
        if self.remediation.sources().is_empty() {
            found.push(SpecificityDefect::RemediationUncited);
        }
        if self.remediation.activity() != RemediationActivity::for_kind(self.kind) {
            found.push(SpecificityDefect::RemediationDoesNotMatchKind);
        }
        if matches!(&self.alternative, AlternativePath::Routes { routes } if routes.is_empty()) {
            found.push(SpecificityDefect::AlternativeIsEmpty);
        }
        if self.linked.is_empty() {
            found.push(SpecificityDefect::NoLinkedContext);
        }
        found
    }

    /// Which of section 15.2's five kinds.
    #[must_use]
    pub const fn kind(&self) -> GapKind {
        self.kind
    }

    /// Field one, `무엇`.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// Field one's tier.
    #[must_use]
    pub const fn subject_kind(&self) -> EntityKind {
        self.subject_kind
    }

    /// Field two, `왜 막는가`.
    #[must_use]
    pub const fn blocks(&self) -> &BlockingPath {
        &self.blocks
    }

    /// Field three, `근거`.
    #[must_use]
    pub fn evidence(&self) -> &[EvidenceId] {
        &self.evidence
    }

    /// Field four, `confidence`.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// Field five, `현재 상태`.
    #[must_use]
    pub const fn current_state(&self) -> &StateSnapshot {
        &self.current_state
    }

    /// Field six, `최소 보강`.
    #[must_use]
    pub const fn remediation(&self) -> &MinimumRemediation {
        &self.remediation
    }

    /// Field seven, `대체 경로`.
    #[must_use]
    pub const fn alternative(&self) -> &AlternativePath {
        &self.alternative
    }

    /// Field eight, `연결된 강의/프로젝트`.
    #[must_use]
    pub const fn linked(&self) -> &LinkedContext {
        &self.linked
    }
}
