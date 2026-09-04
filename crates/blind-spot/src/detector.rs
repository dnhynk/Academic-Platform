//! The one recomputation path, and the order it decides in.
//!
//! ## This is the whole AI-rerun surface
//!
//! Section 25.12: `NOT_RELEVANT는 존중되며 새로운 AI run이 경고를 되살리지
//! 않는다`. A rerun of this engine is a call to [`detect`] with new readings, and
//! [`detect`] is **the only function in this crate that produces a
//! [`crate::finding::BlindSpotFinding`]**. `not_relevant_survives_ai_rerun`
//! measures that rather than assuming it: it extracts every public signature in
//! the package whose return type mentions a finding and compares the set against
//! a pin in both directions, so a second producer added later is a failure until
//! it is listed and driven.
//!
//! The disposition arrives in a [`crate::disposition::DispositionLedger`] that
//! this function reads and never writes, and the ledger's own entries can only
//! have been minted through ADR-003's actor matrix. So the three ways a rerun
//! could resurrect a warning — recording a different disposition, dropping the
//! standing one, or ignoring it — are respectively a claim no model actor can
//! make, a ledger operation that does not exist, and this order's first step.
//!
//! ## The order
//!
//! ```text
//! NOT_RELEVANT standing        -> OUT_OF_SCOPE
//! P2-N5 carried a goal block   -> GAP
//! count below the user minimum -> UNOBSERVED
//! an admitted attempt failed   -> WEAK
//! P2-N3's band is low          -> STALE
//! otherwise                    -> no finding at all
//! ```
//!
//! Two of those steps are decisions rather than readings.
//!
//! **`NOT_RELEVANT` outranks everything, including `GAP`.** Section 3 lists
//! `특정 Blind Spot을 탐색할지 의도적으로 제외할지` among the decisions the user
//! owns, and section 23 says the exclusion is `user scope에서 제외된다` rather
//! than a state the detector overrides. `P2-N5` still reports the gap in its own
//! lane; this view does not reopen a scope the user closed.
//!
//! **A key that is adequately covered, undamaged and fresh produces no finding.**
//! Section 34.5's failure mode for this feature is `Blind Spot을 공부 압박으로
//! 변환`, and a detector that emitted a row per key would be exactly the endless
//! deficit list that failure describes.

use std::collections::BTreeMap;

use academic_domain::TimestampMillis;

use crate::{
    BlindSpotError,
    coverage::FieldCoverage,
    disposition::{DispositionLedger, UserDisposition},
    explanation::SkewExplanation,
    finding::BlindSpotFinding,
    presentation::{FindingPresentation, NeutralPresentation},
    reading::KeyReading,
    resolution::FieldResolver,
    scope::BlindSpotScope,
    state::{
        BelowMinimum, LOW_RECENCY_BANDS, LowRecency, ObservedDifficulty, ScopeExclusion,
        StateBasis, state_of,
    },
    taste::TastePath,
};

/// Reads every key the user selected and reports the ones worth showing.
///
/// `as_of` is the instant the reading is for. This crate reads no clock; every
/// instant it holds arrived as an argument.
///
/// # Errors
///
/// [`BlindSpotError::ItemIsAboutAnotherKey`] and
/// [`BlindSpotError::ItemIsOutsideTheTaxonomy`] from the coverage pass;
/// [`BlindSpotError::DispositionIsAboutAnotherField`] when the ledger's standing
/// choice for a key names another one; [`BlindSpotError::ExploreWithoutAStep`]
/// when the user pressed `EXPLORE` and no taste step was offered for that key;
/// and the refusals each [`StateBasis`] payload makes.
pub fn detect(
    scope: &BlindSpotScope,
    resolver: &FieldResolver,
    ledger: &DispositionLedger,
    readings: &[KeyReading],
    as_of: TimestampMillis,
) -> Result<Vec<BlindSpotFinding>, BlindSpotError> {
    let mut coverage: BTreeMap<_, FieldCoverage> = BTreeMap::new();
    for reading in readings {
        let counted = FieldCoverage::of(reading.key(), scope, resolver, reading.items())?;
        coverage.insert(reading.key(), counted);
    }
    let all: Vec<FieldCoverage> = coverage.values().cloned().collect();
    let cause = SkewExplanation::of(scope, &all);

    let mut findings = Vec::new();
    for reading in readings {
        let Some(counted) = coverage.get(&reading.key()) else {
            continue;
        };
        let standing = ledger.standing(reading.key());
        if let Some(choice) = standing
            && choice.field() != reading.key()
        {
            return Err(BlindSpotError::DispositionIsAboutAnotherField);
        }
        let disposition = standing.map(|choice| choice.disposition());

        let basis = if let Some(choice) =
            standing.filter(|held| held.disposition() == UserDisposition::NotRelevant)
        {
            StateBasis::UserExcluded(ScopeExclusion::of(choice)?)
        } else if let Some(block) = reading.goal_block() {
            StateBasis::ActiveGoalBlocked(block)
        } else if counted.evidence_count() < scope.minimum_exposure() {
            StateBasis::CoverageBelowMinimum(BelowMinimum::of(
                counted.evidence_count(),
                scope.minimum_exposure(),
            )?)
        } else if !counted.failed_attempts().is_empty() {
            StateBasis::DifficultyObserved(ObservedDifficulty::of(
                counted.failed_attempts().to_vec(),
            )?)
        } else if let Some(band) = reading
            .band()
            .filter(|held| LOW_RECENCY_BANDS.contains(held))
        {
            StateBasis::RecencyLow(LowRecency::of(band)?)
        } else {
            continue;
        };

        let relevance = reading.relevance();
        let copy = NeutralPresentation::of(state_of(&basis), relevance.clone());
        let presentation = if disposition == Some(UserDisposition::Explore) {
            let Some(choice) = standing else {
                return Err(BlindSpotError::ExploreWithoutAStep);
            };
            let Some(step) = reading.taste_step() else {
                return Err(BlindSpotError::ExploreWithoutAStep);
            };
            FindingPresentation::Explore {
                presentation: copy,
                path: TastePath::for_explore(choice, reading.key(), step)?,
            }
        } else {
            FindingPresentation::Neutral { presentation: copy }
        };

        let warns = !standing.is_some_and(|choice| choice.suppresses_warning_at(as_of));

        findings.push(BlindSpotFinding::assemble(
            reading.key(),
            scope.label(),
            counted.evidence_count(),
            counted.diversity(),
            basis,
            relevance.clone(),
            cause.clone(),
            disposition,
            presentation,
            warns,
        ));
    }
    Ok(findings)
}
