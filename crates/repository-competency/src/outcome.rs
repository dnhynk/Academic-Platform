//! Section 17.6's third bullet: `test, explanation, debugging, review 등 결과
//! evidence`.
//!
//! Four kinds, and what they do is **strengthen** a candidate that authorship
//! already produced. None of them creates one, and the reason is section 13.2's
//! own table: the rows that reach `Applied candidate` are `직접 작성한
//! production/personal project code와 test` and `incident debugging에서 원인
//! 규명·수정·검증`, and both of them name the user's own work first. An outcome
//! beside somebody else's change is evidence about that change, not about this
//! user's competency.
//!
//! ## Review is here and nowhere else
//!
//! `REVIEW` is an [`OutcomeKind`], and there is no authorship value it can
//! become. [`crate::AuthorshipMode`] enumerates two things — `AUTHORED` and
//! `SUBSTANTIVE_CONTRIBUTION` — and neither is reachable from an outcome:
//! [`crate::ContributionKind::authorship_mode`] is the one door, it is total
//! over its own enumeration with no default arm, and `REVIEWED` and `READ`
//! answer [`None`] there.
//!
//! So a review raises [`CandidateSupport`] and appears in a claim's provenance
//! under its own name, and the field a claim serializes its authorship into can
//! only ever hold one of [`crate::AuthorshipMode::ALL`]'s two spellings.
//!
//! ## The three support levels are section 13.2's rows
//!
//! [`CandidateSupport`] is ordered, and its top two levels are read out of
//! section 13.2's table rather than invented here:
//! `CODE_AND_OUTCOME` is the row whose ceiling is `Applied candidate`, and
//! `DIAGNOSED_FAILURE` is the row whose ceiling is `Applied, transfer facet
//! 강화`. `AUTHORSHIP_ONLY` is what is left when the user's own change carries
//! no outcome beside it yet.

use academic_repository_analysis::Locator;

use crate::{CompetencyError, contribution::ChangeId};

/// Which of section 17.6's four result evidences an artifact is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutcomeKind {
    /// A test that exercises the behaviour.
    Test,
    /// The user's own written account of why it is what it is.
    Explanation,
    /// A diagnosis: cause found, fixed, and the fix checked.
    Debugging,
    /// The user read somebody else's change and said something about it.
    Review,
}

impl OutcomeKind {
    /// Exhaustive order, in section 17.6's own order.
    pub const ALL: [Self; 4] = [Self::Test, Self::Explanation, Self::Debugging, Self::Review];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Test => "TEST",
            Self::Explanation => "EXPLANATION",
            Self::Debugging => "DEBUGGING",
            Self::Review => "REVIEW",
        }
    }
}

/// One piece of result evidence, about one concept, at one place.
///
/// It names the change it is about, so an outcome cannot be counted toward a
/// contribution it has nothing to do with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeArtifact {
    kind: OutcomeKind,
    concept: String,
    change: ChangeId,
    at: Locator,
    recorded_at: u64,
}

impl OutcomeArtifact {
    /// Records one outcome.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when the concept is empty, over
    /// 64 bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(
        kind: OutcomeKind,
        concept: impl Into<String>,
        change: ChangeId,
        at: Locator,
        recorded_at: u64,
    ) -> Result<Self, CompetencyError> {
        Ok(Self {
            kind,
            concept: crate::identity::validated(concept.into(), "concept")?,
            change,
            at,
            recorded_at,
        })
    }

    /// Which kind of outcome.
    #[must_use]
    pub const fn kind(&self) -> OutcomeKind {
        self.kind
    }

    /// Which concept it is evidence about.
    #[must_use]
    pub fn concept(&self) -> &str {
        &self.concept
    }

    /// Which change it is about.
    #[must_use]
    pub const fn change(&self) -> &ChangeId {
        &self.change
    }

    /// Where it is.
    #[must_use]
    pub const fn at(&self) -> &Locator {
        &self.at
    }

    /// When it was recorded.
    #[must_use]
    pub const fn recorded_at(&self) -> u64 {
        self.recorded_at
    }
}

/// How much a candidate is carrying, in section 13.2's own terms.
///
/// Ordered, so a reader can compare two candidates without a second table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidateSupport {
    /// The user's own meaningful change, with no outcome beside it yet.
    AuthorshipOnly,
    /// Section 13.2's `직접 작성한 production/personal project code와 test`,
    /// whose ceiling that table gives as `Applied candidate`.
    CodeAndOutcome,
    /// Section 13.2's `incident debugging에서 원인 규명·수정·검증`, whose
    /// ceiling that table gives as `Applied, transfer facet 강화`.
    DiagnosedFailure,
}

impl CandidateSupport {
    /// Exhaustive order, weakest first.
    pub const ALL: [Self; 3] = [
        Self::AuthorshipOnly,
        Self::CodeAndOutcome,
        Self::DiagnosedFailure,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthorshipOnly => "AUTHORSHIP_ONLY",
            Self::CodeAndOutcome => "CODE_AND_OUTCOME",
            Self::DiagnosedFailure => "DIAGNOSED_FAILURE",
        }
    }

    /// What a set of outcomes raises a candidate to.
    ///
    /// Total over [`OutcomeKind`] with no default arm: `DEBUGGING` reaches the
    /// top level because section 13.2 gives that row its own ceiling, and the
    /// other three reach the middle one. An empty set stays at the floor, which
    /// is the whole of *strengthens rather than creates* — the floor is already
    /// a candidate.
    #[must_use]
    pub fn of(outcomes: &[&OutcomeArtifact]) -> Self {
        let mut level = Self::AuthorshipOnly;
        for outcome in outcomes {
            let reached = match outcome.kind() {
                OutcomeKind::Debugging => Self::DiagnosedFailure,
                OutcomeKind::Test | OutcomeKind::Explanation | OutcomeKind::Review => {
                    Self::CodeAndOutcome
                }
            };
            if reached > level {
                level = reached;
            }
        }
        level
    }
}
