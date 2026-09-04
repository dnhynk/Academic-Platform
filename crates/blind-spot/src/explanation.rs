//! Section 23's `편중의 원인도 설명한다`, as a distribution rather than a
//! sentence.
//!
//! ## Why this is not the example's string
//!
//! Section 23's schema example writes
//! `likelyCause: "course/project choices concentrated in backend"`, and the
//! paragraph below it says what that sentence summarises: `backend repo 세 개와
//! Database/Networks 강의 때문에 Application/Backend evidence가 많고
//! Graphics/Formal Methods가 비어 있음`. That is three counts, two crowded keys
//! and two empty ones.
//!
//! A free-text cause would be the one slot in a finding through which an action
//! demand could arrive, and section 23's last sentence is that when a blind spot
//! is unrelated to the user's goals the product must not make one. A word list
//! cannot hold that — every list admits the sentence spelled differently — so
//! the slot does not exist: [`SkewExplanation`] carries the distribution and the
//! caller renders it. `docs/contracts/blind-spot-detector.md` records the
//! deviation from the example's string.
//!
//! ## Neither bound is a threshold this file chose
//!
//! `concentrated` is the keys holding the **maximum** count, every tie retained,
//! which is `P2-N5`'s `equal candidates are both retained` on a different axis.
//! `sparse` is the keys below the minimum **the user selected** in
//! [`crate::scope::BlindSpotScope`]. There is no third number.

use serde::{Deserialize, Serialize};

use academic_domain::EntityId;

use crate::{
    coverage::{ExposureSource, FieldCoverage},
    scope::BlindSpotScope,
};

/// One `(key, source)` pair and how many items it contributed.
///
/// Section 23's `backend repo 세 개` is one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExposureDriver {
    /// Which aggregation key the items landed on.
    pub key: EntityId,
    /// Which of section 23's five sources they came from.
    pub source: ExposureSource,
    /// How many.
    pub count: u32,
}

/// Why the distribution leans where it leans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkewExplanation {
    drivers: Vec<ExposureDriver>,
    concentrated: Vec<EntityId>,
    sparse: Vec<EntityId>,
}

impl SkewExplanation {
    /// Reads the distribution off every key's coverage under `scope`.
    #[must_use]
    pub fn of(scope: &BlindSpotScope, coverage: &[FieldCoverage]) -> Self {
        let mut drivers: Vec<ExposureDriver> = Vec::new();
        for reading in coverage {
            for (source, count) in reading.by_source() {
                drivers.push(ExposureDriver {
                    key: reading.key(),
                    source: *source,
                    count: *count,
                });
            }
        }
        drivers.sort_unstable();

        let highest = coverage
            .iter()
            .map(FieldCoverage::evidence_count)
            .max()
            .unwrap_or_default();
        let mut concentrated: Vec<EntityId> = coverage
            .iter()
            .filter(|reading| highest > 0 && reading.evidence_count() == highest)
            .map(FieldCoverage::key)
            .collect();
        concentrated.sort_unstable();

        let mut sparse: Vec<EntityId> = coverage
            .iter()
            .filter(|reading| reading.evidence_count() < scope.minimum_exposure())
            .map(FieldCoverage::key)
            .collect();
        sparse.sort_unstable();

        Self {
            drivers,
            concentrated,
            sparse,
        }
    }

    /// Every `(key, source, count)` the distribution rests on.
    #[must_use]
    pub fn drivers(&self) -> &[ExposureDriver] {
        &self.drivers
    }

    /// The keys holding the maximum count, every tie retained.
    #[must_use]
    pub fn concentrated(&self) -> &[EntityId] {
        &self.concentrated
    }

    /// The keys below the minimum the user selected.
    #[must_use]
    pub fn sparse(&self) -> &[EntityId] {
        &self.sparse
    }
}
