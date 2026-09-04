//! Section 20.3's three motivation edges, and the sum that has no spelling.
//!
//! > 동일 concept도 `SCHOOL`, `ROLE`, `PROJECT` motivation edge를 복수로 가질
//! > 수 있다. UI는 이를 합산 점수로 숨기지 않고 “이번 주 project 때문에”, “다음
//! > 강의 prerequisite라서”, “장기 systems path에서 재사용”처럼 병렬로 보여준다.
//!
//! ## `motivation_edges_are_shown_in_parallel`
//!
//! [`MotivationDisplay::rows`] returns one [`MotivationRow`] per edge the
//! concept has, each carrying its own reason, in [`MOTIVATIONS`]' order. There
//! is no other reader: [`MotivationDisplay`] has private fields and hands out
//! nothing but the rows and the count of them.
//!
//! ## Why the absence is stated over the whole crate rather than over this type
//!
//! `P2-N6` measured the folding shape in this repository once — a seven-axis
//! cost vector with a `total()` — and `P2-N7` measured it again in
//! `academic-critical-path`. `P2-X5` then measured the shape that neither of
//! those sweeps could see: **a trait implementation declares no `pub fn`**, so
//! `impl From<FieldCoverage> for u32` summing every source's evidence count
//! passes a whole-crate public-signature inventory untouched. That instance is
//! still open in `academic-blind-spot` and is recorded in
//! `docs/contracts/build-to-learn.md`.
//!
//! So the absence here is three whole-set comparisons in
//! `crates/build-learn/tests/build_learn_scans.rs`, none of which is a list of
//! names:
//!
//! * `every_impl_header_in_this_crate_is_in_the_inventory` pins every `impl`
//!   header the package declares in both directions and then refuses every
//!   conversion trait over the **whole** inventory, for any type pair at all —
//!   so `From<MotivationDisplay> for u32`, `Into`, `Sum`, `Add`, `Deref`,
//!   `AsRef` and `TryFrom` have no place to be added.
//! * `every_public_signature_is_in_the_inventory` pins every public signature,
//!   so a second reader under any name is an inventory entry.
//! * `no_signature_folds_the_motivation_edges` compares the whole set of public
//!   signatures that name a motivation type and return a number against the
//!   empty set, with each half shown separately non-empty and the predicate
//!   shown to bite on a fragment that does fold.
//!
//! And `Motivation` derives no arithmetic trait, holds no numeric payload, and
//! has no `weight`, `score`, `rank` or ordinal accessor. Its `Ord` is
//! declaration order, which is what makes the rows presentable in a fixed order
//! without making one of them larger than another.

use std::collections::BTreeSet;

use academic_domain::EntityId;
use serde::{Deserialize, Serialize};

use crate::{BuildLearnError, text::NonEmptyText};

/// Section 20.3's three motivation labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Motivation {
    /// `SCHOOL`: `다음 강의 prerequisite라서`.
    School,
    /// `ROLE`: `장기 systems path에서 재사용`.
    Role,
    /// `PROJECT`: `이번 주 project 때문에`.
    Project,
}

/// The three, in the design document's own order.
pub const MOTIVATIONS: [Motivation; 3] =
    [Motivation::School, Motivation::Role, Motivation::Project];

impl Motivation {
    /// The design document's own label.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::School => "SCHOOL",
            Self::Role => "ROLE",
            Self::Project => "PROJECT",
        }
    }

    /// Stable spelling. The same text; the label is already the wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.spec_token()
    }
}

/// One motivation edge: why this concept is on the plan under one label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotivationEdge {
    motivation: Motivation,
    concept: EntityId,
    reason: NonEmptyText,
}

impl MotivationEdge {
    /// Records one edge.
    #[must_use]
    pub const fn of(motivation: Motivation, concept: EntityId, reason: NonEmptyText) -> Self {
        Self {
            motivation,
            concept,
            reason,
        }
    }

    /// Which of the three labels.
    #[must_use]
    pub const fn motivation(&self) -> Motivation {
        self.motivation
    }

    /// The concept it is about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The reason, in the user's terms.
    #[must_use]
    pub const fn reason(&self) -> &NonEmptyText {
        &self.reason
    }
}

/// One row of the parallel display: one label and its own reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotivationRow {
    motivation: Motivation,
    reason: NonEmptyText,
}

impl MotivationRow {
    /// Which of the three labels this row is.
    #[must_use]
    pub const fn motivation(&self) -> Motivation {
        self.motivation
    }

    /// The reason shown beside it.
    #[must_use]
    pub const fn reason(&self) -> &NonEmptyText {
        &self.reason
    }
}

/// Every motivation one concept carries, side by side.
///
/// Private fields, one producer, no `Default`. See the module note for what is
/// deliberately absent and where that absence is measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotivationDisplay {
    concept: EntityId,
    rows: Vec<MotivationRow>,
}

impl MotivationDisplay {
    /// Gathers the edges of one concept into parallel rows.
    ///
    /// Rows come out in [`MOTIVATIONS`]' order whatever order the edges arrived
    /// in, so the display is stable without any of the three being ranked above
    /// another.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::MotivationEdgeIsAboutAnotherConcept`] when an edge
    /// names a different concept, and
    /// [`BuildLearnError::DuplicateMotivationEdge`] when one label arrives
    /// twice — two reasons under one label would have to be joined into one row
    /// or ranked against each other, and section 20.3's sentence is that neither
    /// happens.
    pub fn of(concept: EntityId, edges: &[MotivationEdge]) -> Result<Self, BuildLearnError> {
        let mut seen: BTreeSet<Motivation> = BTreeSet::new();
        for edge in edges {
            if edge.concept() != concept {
                return Err(BuildLearnError::MotivationEdgeIsAboutAnotherConcept {
                    expected: concept.to_string(),
                    found: edge.concept().to_string(),
                });
            }
            if !seen.insert(edge.motivation()) {
                return Err(BuildLearnError::DuplicateMotivationEdge(
                    edge.motivation().as_str(),
                ));
            }
        }
        let rows = MOTIVATIONS
            .iter()
            .filter_map(|motivation| {
                edges
                    .iter()
                    .find(|edge| edge.motivation() == *motivation)
                    .map(|edge| MotivationRow {
                        motivation: *motivation,
                        reason: edge.reason().clone(),
                    })
            })
            .collect();
        Ok(Self { concept, rows })
    }

    /// The concept the rows are about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The rows, in [`MOTIVATIONS`]' order. One per label the concept carries.
    #[must_use]
    pub fn rows(&self) -> &[MotivationRow] {
        &self.rows
    }

    /// Whether this concept carries `motivation`.
    #[must_use]
    pub fn carries(&self, motivation: Motivation) -> bool {
        self.rows.iter().any(|row| row.motivation() == motivation)
    }
}
