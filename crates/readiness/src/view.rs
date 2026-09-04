//! The readiness view: the matrix, the notice, and the score that comes after
//! both.
//!
//! ## The matrix is the default view, because it is the only one
//!
//! [`ReadinessView::of`] is the single producer of a view, it takes no mode, no
//! preference and no flag, and [`ReadinessView::render`] always emits
//! [`ViewBlock::Matrix`] **first**. There is no second constructor, no summary
//! view and no toggle: `matrix_is_the_default_view` compares the whole set of
//! public functions returning a view against that one, and walks a generated
//! cross-product of matrices, scores and histories requiring block zero to be
//! the matrix in every one of them.
//!
//! ## No aggregate percentage is the primary output
//!
//! This is an absence, so it is stated as three whole-set comparisons rather
//! than as a list of forbidden names — a list refuses the spellings somebody
//! thought of and admits every other one, which is the second of the three
//! shapes `docs/contracts/policy-source-scans.md` calls an empty guard.
//!
//! 1. **Position.** Block zero of every rendered view is the matrix. A block
//!    placed before it fails whatever it is called.
//! 2. **The block vocabulary is closed.** [`ViewBlock::KINDS`] is compared
//!    against the enumeration in both directions, so a new kind of block fails
//!    until somebody writes it down.
//! 3. **The field vocabulary is closed.** `every_field_of_this_crate_is_in_the_inventory`
//!    compares every declared field of every type in this crate against a pinned
//!    inventory in both directions and requires each to say what it holds, and
//!    it refuses `f32` and `f64` outright — so a ratio has no type to arrive in,
//!    whatever the field is called, and the one cross-row aggregate this crate
//!    has, [`crate::ScoreValue`], is required to appear in exactly one field of
//!    exactly one type.
//!
//! ## Reading a view back is not a door
//!
//! [`ReadinessView`] is `Serialize` and not `Deserialize`, for `P2-Y1`'s reason:
//! a view read back out of a document would be a filled matrix and a published
//! score that ran neither producer. What a recipient of a published document
//! gets instead is [`published_notice`], which refuses a document whose notice
//! is absent or is not the one [`crate::NonGuaranteeNotice`] renders.

use std::collections::BTreeMap;

use academic_competency::{Competency, CompetencyId, CriterionId, PerformanceCriterion};
use serde::Serialize;

use crate::{
    ReadinessError,
    history::ReadinessEvent,
    matrix::ReadinessMatrix,
    notice::NonGuaranteeNotice,
    score::{
        AuxiliaryScore, MissingDataDisclosure, RubricDisclosure, SourceDisclosure,
        WeightDisclosure, disclose,
    },
};

/// The JSON key a published document carries its notice under.
pub const NOTICE_KEY: &str = "nonGuaranteeNotice";

/// One block of a rendered view, in the order a reader meets them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "block", content = "body", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ViewBlock<'view> {
    /// Section 24.3's matrix. Always block zero.
    Matrix(&'view ReadinessMatrix),
    /// The non-guarantee notice.
    NonGuarantee(NonGuaranteeNotice),
    /// The auxiliary score, when one has been published and not hidden.
    AuxiliaryScore(&'view AuxiliaryScore),
    /// What has happened to the score.
    History(&'view [ReadinessEvent]),
}

impl ViewBlock<'_> {
    /// Stable spelling of which block this is.
    ///
    /// Total, with no wildcard arm: a fifth block has to answer this rather
    /// than inherit an answer.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Matrix(_) => "MATRIX",
            Self::NonGuarantee(_) => "NON_GUARANTEE",
            Self::AuxiliaryScore(_) => "AUXILIARY_SCORE",
            Self::History(_) => "HISTORY",
        }
    }

    /// Every kind's spelling, in this enumeration's own order.
    pub const KINDS: [&'static str; 4] = ["MATRIX", "NON_GUARANTEE", "AUXILIARY_SCORE", "HISTORY"];
}

/// Section 24.3's readiness view.
///
/// No public field, no setter and no `&mut self` method. Every change is a new
/// view over the old one, and the old one keeps its own history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessView {
    matrix: ReadinessMatrix,
    non_guarantee_notice: NonGuaranteeNotice,
    score: Option<AuxiliaryScore>,
    history: Vec<ReadinessEvent>,
    #[serde(skip)]
    criteria: BTreeMap<CompetencyId, Vec<CriterionId>>,
}

impl ReadinessView {
    /// Opens the view of one matrix.
    ///
    /// The only producer. There is no mode argument and no second door, which
    /// is what makes the matrix the *default* view rather than one of several.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::DisclosureDoesNotCoverTheMatrix`] when a row of the
    /// matrix has no competency in `competencies`, because a view whose rows
    /// cannot be drilled into is section 24.4's abstract phrase in another
    /// shape.
    pub fn of(
        matrix: ReadinessMatrix,
        competencies: &[&Competency],
    ) -> Result<Self, ReadinessError> {
        let mut criteria = BTreeMap::new();
        for row in matrix.rows() {
            let competency = competencies
                .iter()
                .find(|item| item.id() == row.competency())
                .ok_or_else(|| {
                    ReadinessError::DisclosureDoesNotCoverTheMatrix(
                        "criteria",
                        row.competency().as_str().to_owned(),
                    )
                })?;
            criteria.insert(
                row.competency().clone(),
                competency
                    .criteria()
                    .iter()
                    .map(|item| item.id().clone())
                    .collect(),
            );
        }
        Ok(Self {
            matrix,
            non_guarantee_notice: NonGuaranteeNotice::rendered(),
            score: None,
            history: Vec::new(),
            criteria,
        })
    }

    /// The matrix.
    #[must_use]
    pub const fn matrix(&self) -> &ReadinessMatrix {
        &self.matrix
    }

    /// The notice.
    #[must_use]
    pub const fn notice(&self) -> NonGuaranteeNotice {
        self.non_guarantee_notice
    }

    /// The published score, when one is displayed.
    #[must_use]
    pub const fn score(&self) -> Option<&AuxiliaryScore> {
        self.score.as_ref()
    }

    /// Everything that has happened to the score, oldest first.
    #[must_use]
    pub fn history(&self) -> &[ReadinessEvent] {
        &self.history
    }

    /// One competency's criteria, in the order it states them.
    ///
    /// Empty for a competency this view is not of, which cannot happen for a
    /// row: [`Self::of`] refuses a matrix whose rows it was given no competency
    /// for.
    #[must_use]
    pub fn criteria_of(&self, competency: &CompetencyId) -> &[CriterionId] {
        self.criteria
            .get(competency)
            .map_or(&[] as &[CriterionId], Vec::as_slice)
    }

    /// The blocks a reader meets, in order.
    ///
    /// Block zero is the matrix in every view this crate can produce. The score
    /// appears only when one is published, and never before the notice.
    #[must_use]
    pub fn render(&self) -> Vec<ViewBlock<'_>> {
        let mut blocks = vec![
            ViewBlock::Matrix(&self.matrix),
            ViewBlock::NonGuarantee(self.non_guarantee_notice),
        ];
        if let Some(score) = self.score.as_ref() {
            blocks.push(ViewBlock::AuxiliaryScore(score));
        }
        if !self.history.is_empty() {
            blocks.push(ViewBlock::History(&self.history));
        }
        blocks
    }

    /// Publishes one auxiliary score over this view's matrix.
    ///
    /// Returns a **new** view. This one is not touched.
    ///
    /// # Errors
    ///
    /// Whatever [`disclose`] refuses: a disclosure that is not the one this
    /// matrix produces, or a weighting that is not total over the evidence
    /// columns.
    pub fn publish_score(
        &self,
        competencies: &[&Competency],
        rubric: RubricDisclosure,
        sources: SourceDisclosure,
        missing_data: MissingDataDisclosure,
        weights: WeightDisclosure,
    ) -> Result<Self, ReadinessError> {
        let score = disclose(
            &self.matrix,
            competencies,
            rubric,
            sources,
            missing_data,
            weights,
        )?;
        let mut history = self.history.clone();
        history.push(ReadinessEvent::ScorePublished {
            value: score.value(),
            weights: score.weights().clone(),
        });
        Ok(Self {
            matrix: self.matrix.clone(),
            non_guarantee_notice: self.non_guarantee_notice,
            score: Some(score),
            history,
            criteria: self.criteria.clone(),
        })
    }

    /// Section 34.5's `score 숨김`.
    ///
    /// Returns a **new** view with no displayed score and one more history
    /// entry. This one is not touched, and the hidden number stays openable in
    /// the new view's history.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::NoScoreIsDisplayed`] when there is nothing to hide,
    /// because an event recording that a score was hidden when none was
    /// displayed would be a history entry that did not happen.
    pub fn hide_score(&self) -> Result<Self, ReadinessError> {
        let score = self
            .score
            .as_ref()
            .ok_or(ReadinessError::NoScoreIsDisplayed)?;
        let mut history = self.history.clone();
        history.push(ReadinessEvent::ScoreHidden {
            value: score.value(),
            weights: score.weights().clone(),
        });
        Ok(Self {
            matrix: self.matrix.clone(),
            non_guarantee_notice: self.non_guarantee_notice,
            score: None,
            history,
            criteria: self.criteria.clone(),
        })
    }

    /// Section 34.5's `가중치 초기화`.
    ///
    /// Returns a **new** view whose score is recomputed under `weights`, with
    /// the old weighting and the old number recorded. This one is not touched.
    ///
    /// # Errors
    ///
    /// [`ReadinessError::NoScoreIsDisplayed`] when no score is displayed, and
    /// whatever [`disclose`] refuses for the new weighting.
    pub fn reset_weights(
        &self,
        competencies: &[&Competency],
        weights: WeightDisclosure,
    ) -> Result<Self, ReadinessError> {
        let current = self
            .score
            .as_ref()
            .ok_or(ReadinessError::NoScoreIsDisplayed)?;
        let previous_weights = current.weights().clone();
        let previous_value = current.value();
        let score = disclose(
            &self.matrix,
            competencies,
            RubricDisclosure::of(&self.matrix, competencies)?,
            SourceDisclosure::of(&self.matrix),
            MissingDataDisclosure::of(&self.matrix),
            weights,
        )?;
        let mut history = self.history.clone();
        history.push(ReadinessEvent::WeightsReset {
            from: previous_weights,
            to: score.weights().clone(),
            previous_value,
        });
        Ok(Self {
            matrix: self.matrix.clone(),
            non_guarantee_notice: self.non_guarantee_notice,
            score: Some(score),
            history,
            criteria: self.criteria.clone(),
        })
    }
}

/// Reads the notice out of a published readiness document.
///
/// The recipient's side of the export contract: somebody holding only the bytes
/// gets a notice or gets an error, and there is no third outcome. The
/// comparison is against [`NonGuaranteeNotice::rendered`], which has one
/// producer and no argument, so this is not a check against a constant a caller
/// could have supplied a different value beside.
///
/// # Errors
///
/// [`ReadinessError::NoticeIsMissing`] when the document carries no
/// [`NOTICE_KEY`], and [`ReadinessError::NoticeDoesNotMatch`] when it carries
/// one that is not the rendered notice.
pub fn published_notice(document: &str) -> Result<NonGuaranteeNotice, ReadinessError> {
    let expected = NonGuaranteeNotice::rendered();
    let needle = format!("\"{NOTICE_KEY}\":");
    let start = document
        .find(&needle)
        .ok_or(ReadinessError::NoticeIsMissing)?
        + needle.len();
    let rest = document[start..].trim_start();
    let quoted = rest
        .strip_prefix('"')
        .ok_or(ReadinessError::NoticeDoesNotMatch)?;
    let end = quoted.find('"').ok_or(ReadinessError::NoticeDoesNotMatch)?;
    if quoted[..end] == expected.text() {
        Ok(expected)
    } else {
        Err(ReadinessError::NoticeDoesNotMatch)
    }
}

/// The criteria of one competency, for callers assembling a view's index.
///
/// A thin reading of `P2-Y1`'s own value, here so that a caller does not have
/// to reach into [`PerformanceCriterion`] to find out what a row can be drilled
/// into.
#[must_use]
pub fn criteria_of(competency: &Competency) -> Vec<&PerformanceCriterion> {
    competency.criteria().iter().collect()
}
