//! Section 23's `taxonomy granularity와 기간을 사용자가 선택한다`.
//!
//! ## Nothing here has a shipped value
//!
//! [`BlindSpotScope`] implements no `Default`, no constant of the type exists in
//! this crate, and [`BlindSpotScope::select`] takes all four choices by value.
//! `P2-N3` holds `GATE-38-024`'s personalization half the same way —
//! `PersonalizationSpeed` has no `Default` and no shipped constant — and
//! `P2-N1` holds the base taxonomy mix the same way again. A scope this crate
//! could produce on its own would be this crate deciding, for the user, which
//! fields count as unobserved and over what stretch of their life.
//!
//! The fourth choice is the one section 23 leaves least specified and it is the
//! one that decides the most. `evidence가 거의 없어` is a threshold, `t001`
//! flagged it as a gate candidate, and section 23 fixes no number — so it is the
//! user's, in the same value that carries the granularity and the window, rather
//! than a constant here.
//!
//! ## Granularity is section 7.4's tier, not a fourth spelling
//!
//! `P2-N1` owns `Field`, `Concept` and `Operation` and maps each onto
//! `EntityKind`. [`TaxonomyGranularity`] is those three and
//! [`TaxonomyGranularity::tier`] returns that crate's own value, so a fourth
//! tier is a change to the ontology rather than an option added here.
//! `EntityKind::ConceptSense` and `EntityKind::Alias` are absent because
//! `P2-N1` has neither as a primary node type.

use serde::{Deserialize, Serialize};

use academic_domain::{
    TimestampMillis, entity_registry::EntityKind, ontology::TaxonomyVersionIdentity,
};

use crate::BlindSpotError;

/// The tier coverage is aggregated at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaxonomyGranularity {
    /// `P2-N1`'s broad cluster.
    Field,
    /// `P2-N1`'s independently explainable unit.
    Concept,
    /// `P2-N1`'s named procedure.
    Operation,
}

/// The three, in the order `P2-N1`'s table declares them.
pub const GRANULARITIES: [TaxonomyGranularity; 3] = [
    TaxonomyGranularity::Field,
    TaxonomyGranularity::Concept,
    TaxonomyGranularity::Operation,
];

impl TaxonomyGranularity {
    /// The `P2-C3` tier this granularity is.
    ///
    /// Total, with no wildcard arm.
    #[must_use]
    pub const fn tier(self) -> EntityKind {
        match self {
            Self::Field => EntityKind::Field,
            Self::Concept => EntityKind::Concept,
            Self::Operation => EntityKind::Operation,
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Field => "FIELD",
            Self::Concept => "CONCEPT",
            Self::Operation => "OPERATION",
        }
    }
}

/// The stretch of the user's record coverage is counted over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservationWindow {
    /// Section 23's example scope, whose second half reads `all-time`.
    AllTime,
    /// A half-open interval, `from` inclusive and `to` exclusive.
    Between {
        /// Inclusive lower bound.
        from: TimestampMillis,
        /// Exclusive upper bound.
        to: TimestampMillis,
    },
}

impl ObservationWindow {
    /// A bounded window.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::WindowIsEmpty`] when `to` is not after `from`.
    pub fn between(from: TimestampMillis, to: TimestampMillis) -> Result<Self, BlindSpotError> {
        if to.value() <= from.value() {
            return Err(BlindSpotError::WindowIsEmpty);
        }
        Ok(Self::Between { from, to })
    }

    /// Whether `at` falls inside the window.
    ///
    /// Total, with no wildcard arm.
    #[must_use]
    pub fn holds(self, at: TimestampMillis) -> bool {
        match self {
            Self::AllTime => true,
            Self::Between { from, to } => from.value() <= at.value() && at.value() < to.value(),
        }
    }

    /// The token section 23's example scope string writes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllTime => "all-time",
            Self::Between { .. } => "bounded",
        }
    }
}

/// Everything the user chose before a single field was read.
///
/// No `Default`, no shipped constant, and all four choices by value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlindSpotScope {
    taxonomy: TaxonomyVersionIdentity,
    granularity: TaxonomyGranularity,
    window: ObservationWindow,
    minimum_exposure: u32,
}

impl BlindSpotScope {
    /// Records the user's four choices.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::MinimumExposureIsZero`] for a minimum of zero, which
    /// is a scope under which no field can ever be `UNOBSERVED` and therefore a
    /// detector that says nothing.
    pub fn select(
        taxonomy: TaxonomyVersionIdentity,
        granularity: TaxonomyGranularity,
        window: ObservationWindow,
        minimum_exposure: u32,
    ) -> Result<Self, BlindSpotError> {
        if minimum_exposure == 0 {
            return Err(BlindSpotError::MinimumExposureIsZero);
        }
        Ok(Self {
            taxonomy,
            granularity,
            window,
            minimum_exposure,
        })
    }

    /// The exact taxonomy version the user selected.
    #[must_use]
    pub const fn taxonomy(&self) -> &TaxonomyVersionIdentity {
        &self.taxonomy
    }

    /// The tier coverage is aggregated at.
    #[must_use]
    pub const fn granularity(&self) -> TaxonomyGranularity {
        self.granularity
    }

    /// The stretch coverage is counted over.
    #[must_use]
    pub const fn window(&self) -> ObservationWindow {
        self.window
    }

    /// How many admitted items a field needs before its exposure is readable.
    #[must_use]
    pub const fn minimum_exposure(&self) -> u32 {
        self.minimum_exposure
    }

    /// Section 23's `scope:` line, assembled from the two halves it names.
    ///
    /// The example reads `undergraduate CS breadth v2, all-time`: the release
    /// identifier of the selected taxonomy version, then the window token.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}, {}", self.taxonomy.release(), self.window.as_str())
    }
}
