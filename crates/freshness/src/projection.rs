//! Section 13.3's projection: seven inputs, six bands, and one value handed
//! back to `P2-N2`.
//!
//! ## The output is a `FreshnessInput` and there is no second channel
//!
//! [`FreshnessProjection::input`] returns `academic_knowledge_state::
//! FreshnessInput`, which carries a band and a confidence and has no third
//! field. Every `P2-N2` entry point that appends an assertion version takes one
//! of those and recomputes the mastery from the surviving evidence, so a band
//! moving cannot move a level: there is no argument through which it could, and
//! this crate has no name for the value it would have to supply.
//! `stale_does_not_demote` and `time_decay_touches_freshness_only` observe both
//! halves.
//!
//! ## What raises a band, what caps it, and which wins
//!
//! Three inputs can **raise** a band — this concept's own dated evidence, a
//! bounded spillover, and the user saying `지금도 바로 사용할 수 있음`. Two can
//! **cap** it — the user saying `복습 필요`, and section 13.3's contrary events.
//!
//! When the latest cap is at least as recent as every raiser, the cap applies.
//! That is `REQ-13-030`'s own case — `recent positive evidence 후 recall-failure
//! event → freshness가 상승하지 않음` — and it is what makes
//! `recall_failure_prevents_freshness_increase` a property rather than one
//! comparison: adding *any* number of further raisers dated at or before the
//! failure changes nothing, because the cap is applied after they are all
//! combined. Relearning still works, because a raiser dated **after** the
//! failure is more recent than it and the cap no longer holds.
//!
//! ## `estimateConfidence` is not this either
//!
//! `P2-N2` fixed that `estimateConfidence` is evidence sufficiency and not a
//! skill score. [`FreshnessProjection::confidence`] is the same kind of value
//! for the other axis: how well-founded *this band* is, carrying
//! [`ConfidenceGap`]s that say what is missing. Its scale is the design
//! document's own. Section 13.1's schema example reads
//! `freshnessBand: VERY_HIGH` with `freshnessConfidence: 0.92` over four
//! evidence items, no contradiction and no recall history — a state with exactly
//! one thing missing, the calibration section 13.3 says the prior is waiting
//! for. So one gap costs 80 permille, and
//! `the_confidence_scale_is_the_schema_examples` reproduces that case.

use academic_domain::{ConfidencePermille, EntityId, FreshnessBand, TimestampMillis};
use academic_knowledge_state::FreshnessInput;
use serde::{Deserialize, Serialize};

use crate::{
    FreshnessError,
    band::{FreshnessSignal, ceiling_of, floor_of, rank},
    decay::decay,
    evidence::{DatedEvidence, Repetition},
    persistence::{PersistenceClass, PersistenceWindow, PriorIdentity, RetentionPrior},
    recall::{ContraryEvent, RecallStatement, UserRecall},
    spillover::Spillover,
};

/// What one gap in the evidence for a band costs, in permille.
///
/// Section 13.1's schema example is the derivation, not a tuning constant: it
/// shows `freshnessConfidence: 0.92` for a concept with four evidence items, an
/// empty `contradictingEvidence` list and no recall history — one open gap, and
/// 1000 less 80.
pub const FRESHNESS_GAP_PERMILLE: u16 = 80;

/// What is missing from the evidence for a band.
///
/// Like `P2-N2`'s `SufficiencyGap`, a low confidence says *what is absent*
/// rather than *how stale the user is*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfidenceGap {
    /// The prior is still `UNCALIBRATED_PRIOR_V1`: `GATE-38-024`.
    PriorUncalibrated,
    /// Nothing datable about this concept was admitted, so the band rests on a
    /// neighbour's use or on nothing.
    NoDirectEvidence,
    /// A contrary event or a `복습 필요` is capping the band.
    ContraryEvidenceStanding,
    /// Only one occasion is in the recent window, so there is no interval to
    /// read — section 13.3's second input contributed nothing.
    NoRepetitionInterval,
}

impl ConfidenceGap {
    /// Exhaustive.
    pub const ALL: [Self; 4] = [
        Self::PriorUncalibrated,
        Self::NoDirectEvidence,
        Self::ContraryEvidenceStanding,
        Self::NoRepetitionInterval,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PriorUncalibrated => "PRIOR_UNCALIBRATED",
            Self::NoDirectEvidence => "NO_DIRECT_EVIDENCE",
            Self::ContraryEvidenceStanding => "CONTRARY_EVIDENCE_STANDING",
            Self::NoRepetitionInterval => "NO_REPETITION_INTERVAL",
        }
    }
}

/// One line of the trace: which of section 13.3's inputs contributed, and what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEntry {
    signal: FreshnessSignal,
    band: Option<FreshnessBand>,
    at: Option<TimestampMillis>,
    detail: String,
}

impl TraceEntry {
    /// Which of section 13.3's seven inputs.
    #[must_use]
    pub const fn signal(&self) -> FreshnessSignal {
        self.signal
    }

    /// The band this input offered or imposed, when it named one.
    #[must_use]
    pub const fn band(&self) -> Option<FreshnessBand> {
        self.band
    }

    /// The instant it rests on, when it has one.
    #[must_use]
    pub const fn at(&self) -> Option<TimestampMillis> {
        self.at
    }

    /// What it was.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// The trace of one projection, in section 13.3's own bullet order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessTrace(Vec<TraceEntry>);

impl FreshnessTrace {
    /// Every entry, in section 13.3's bullet order.
    #[must_use]
    pub fn entries(&self) -> &[TraceEntry] {
        &self.0
    }

    /// Whether one of section 13.3's inputs contributed at all.
    #[must_use]
    pub fn names(&self, signal: FreshnessSignal) -> bool {
        self.0.iter().any(|entry| entry.signal() == signal)
    }

    /// The entries for one input.
    #[must_use]
    pub fn of(&self, signal: FreshnessSignal) -> Vec<&TraceEntry> {
        self.0
            .iter()
            .filter(|entry| entry.signal() == signal)
            .collect()
    }
}

/// One concept's freshness, with everything that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessProjection {
    concept: EntityId,
    band: FreshnessBand,
    confidence: ConfidencePermille,
    as_of: TimestampMillis,
    last_strong_evidence: Option<TimestampMillis>,
    prior: PriorIdentity,
    prior_origin: PriorIdentity,
    prior_uncalibrated: bool,
    gaps: Vec<ConfidenceGap>,
    trace: FreshnessTrace,
}

impl FreshnessProjection {
    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The band.
    #[must_use]
    pub const fn band(&self) -> FreshnessBand {
        self.band
    }

    /// How well-founded the band is.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// The instant this was projected for.
    #[must_use]
    pub const fn as_of(&self) -> TimestampMillis {
        self.as_of
    }

    /// When the most recent admitted evidence about this concept was, if any.
    #[must_use]
    pub const fn last_strong_evidence(&self) -> Option<TimestampMillis> {
        self.last_strong_evidence
    }

    /// The prior this used.
    #[must_use]
    pub const fn prior(&self) -> PriorIdentity {
        self.prior
    }

    /// The prior it was calibrated from, which is the shipped default until one
    /// is.
    #[must_use]
    pub const fn prior_origin(&self) -> PriorIdentity {
        self.prior_origin
    }

    /// Whether the prior behind this band is still the shipped, uncalibrated
    /// one.
    ///
    /// `GATE-38-024`: a caller rendering a band reads this rather than comparing
    /// a name, so the shipped default is visibly labelled wherever it is shown.
    #[must_use]
    pub const fn prior_is_uncalibrated(&self) -> bool {
        self.prior_uncalibrated
    }

    /// What is missing from the evidence for this band.
    #[must_use]
    pub fn gaps(&self) -> &[ConfidenceGap] {
        &self.gaps
    }

    /// Which of section 13.3's inputs contributed, and what.
    #[must_use]
    pub const fn trace(&self) -> &FreshnessTrace {
        &self.trace
    }

    /// The value `P2-N2` takes.
    ///
    /// **This is the only thing this crate hands to the knowledge state.** It
    /// carries a band and a confidence and nothing else, so nothing here can
    /// reach a mastery level even by mistake.
    #[must_use]
    pub const fn input(&self) -> FreshnessInput {
        FreshnessInput::of(self.band, self.confidence)
    }
}

/// Everything section 13.3 lists as an input, for one concept at one instant.
///
/// Every field is a slice the caller owns; nothing here reads a clock, and
/// `as_of` is an argument like every other.
#[derive(Debug, Clone, Copy)]
pub struct FreshnessInputs<'a> {
    /// This concept's own admitted evidence, dated.
    pub dated: &'a [DatedEvidence],
    /// Bounded contributions from related concepts.
    pub spillover: &'a [Spillover],
    /// The user's own recall statements.
    pub statements: &'a [RecallStatement],
    /// Section 13.3's contrary events.
    pub contrary: &'a [ContraryEvent],
}

/// Projects one concept's freshness at `as_of`.
///
/// # Errors
///
/// [`FreshnessError::EvidenceNamesAnotherConcept`] when a dated item, a
/// statement or a contrary event is about a different concept — the same
/// misattribution `P2-N2` closed one layer up;
/// [`FreshnessError::SpilloverNamesAnotherConcept`] when a contribution was
/// computed toward a different concept; [`FreshnessError::InputAfterAsOf`] when
/// an input is dated after the instant being asked about; and
/// [`FreshnessError::Domain`] when the confidence value is out of range.
pub fn project(
    concept: EntityId,
    inputs: FreshnessInputs<'_>,
    prior: &RetentionPrior,
    as_of: TimestampMillis,
) -> Result<FreshnessProjection, FreshnessError> {
    require_about(concept, inputs)?;
    require_not_in_future(inputs, as_of)?;

    let mut trace: Vec<TraceEntry> = Vec::new();

    // Input two, which extends the window input one is measured against.
    let recent = prior.window_of(PersistenceClass::ApplicationOrDesign);
    let repetition = Repetition::over(inputs.dated, recent, as_of);
    if repetition.repeats() > 0 {
        trace.push(TraceEntry {
            signal: FreshnessSignal::RepetitionAndInterval,
            band: None,
            at: None,
            detail: format!(
                "{} occasions across {} days in the last {} days",
                repetition.occasions(),
                repetition.span_days(),
                recent.days()
            ),
        });
    }

    // Input one, read through input three's per-kind window.
    let mut band = FreshnessBand::Unknown;
    let mut last_strong: Option<TimestampMillis> = None;
    let mut latest_raiser: Option<TimestampMillis> = None;
    for item in inputs.dated {
        let elapsed = elapsed_or_refuse(item.occurred_at(), as_of)?;
        let window = extended(item.window(prior), repetition.repeats());
        let contributed = decay(elapsed, window);
        band = ceiling_of(band, contributed);
        last_strong = Some(later_of(last_strong, item.occurred_at()));
        latest_raiser = Some(later_of(latest_raiser, item.occurred_at()));
        trace.push(TraceEntry {
            signal: FreshnessSignal::LastStrongEvidence,
            band: Some(contributed),
            at: Some(item.occurred_at()),
            detail: format!("{:?}", item.kind()),
        });
        trace.push(TraceEntry {
            signal: FreshnessSignal::EvidenceTypePersistence,
            band: None,
            at: None,
            detail: format!("{:?} over {} days", item.kind(), window.days()),
        });
    }

    // Input four. Combined with the higher of the two rather than summed, so a
    // concept with ten neighbours gets what a concept with one gets.
    for contribution in inputs.spillover {
        band = ceiling_of(band, contribution.band());
        latest_raiser = Some(later_of(latest_raiser, contribution.at()));
        trace.push(TraceEntry {
            signal: FreshnessSignal::RelatedConceptSpillover,
            band: Some(contribution.band()),
            at: Some(contribution.at()),
            detail: format!(
                "{} at {:?} across {}",
                contribution.neighbor().as_uuid(),
                contribution.neighbor_band(),
                contribution.edge().predicate().as_str()
            ),
        });
    }

    // Input five's raising half: the user's own `지금도 바로 사용할 수 있음`,
    // which decays like everything else so a statement made years ago raises
    // nothing today.
    let statement_window = prior.window_of(PersistenceClass::ApplicationOrDesign);
    for statement in inputs.statements {
        if !statement.statement().raises() {
            continue;
        }
        let elapsed = elapsed_or_refuse(statement.stated_at(), as_of)?;
        let contributed = floor_of(
            statement.statement().band(),
            decay(elapsed, statement_window),
        );
        band = ceiling_of(band, contributed);
        latest_raiser = Some(later_of(latest_raiser, statement.stated_at()));
        trace.push(TraceEntry {
            signal: FreshnessSignal::UserRecallStatement,
            band: Some(contributed),
            at: Some(statement.stated_at()),
            detail: UserRecall::CanUseNow.phrase().to_owned(),
        });
    }

    // Inputs five and seven's capping half.
    let cap = latest_cap(inputs);
    let capped =
        cap.filter(|(_, at, _)| latest_raiser.is_none_or(|raiser| at.value() >= raiser.value()));
    if let Some((signal, at, ceiling)) = capped {
        band = floor_of(band, ceiling);
        trace.push(TraceEntry {
            signal,
            band: Some(ceiling),
            at: Some(at),
            detail: "caps the band: no input is more recent".to_owned(),
        });
    }

    // Input six is always present, because a band is always read through a
    // prior — and while that prior is the shipped one, the trace says so.
    trace.push(TraceEntry {
        signal: FreshnessSignal::RetentionPriorAndCalibration,
        band: None,
        at: None,
        detail: format!(
            "{} generation {}{}",
            prior.identity().name_str(),
            prior.identity().generation(),
            if prior.is_uncalibrated() {
                " (UNCALIBRATED)"
            } else {
                ""
            }
        ),
    });

    let mut gaps = Vec::new();
    if prior.is_uncalibrated() {
        gaps.push(ConfidenceGap::PriorUncalibrated);
    }
    if inputs.dated.is_empty() {
        gaps.push(ConfidenceGap::NoDirectEvidence);
    }
    if capped.is_some() {
        gaps.push(ConfidenceGap::ContraryEvidenceStanding);
    }
    if repetition.repeats() == 0 {
        gaps.push(ConfidenceGap::NoRepetitionInterval);
    }

    let deduction =
        FRESHNESS_GAP_PERMILLE.saturating_mul(u16::try_from(gaps.len()).unwrap_or(u16::MAX));
    let confidence = ConfidencePermille::new(1000u16.saturating_sub(deduction))?;

    trace.sort_by_key(TraceEntry::signal);
    Ok(FreshnessProjection {
        concept,
        band,
        confidence,
        as_of,
        last_strong_evidence: last_strong,
        prior: prior.identity(),
        prior_origin: prior.origin(),
        prior_uncalibrated: prior.is_uncalibrated(),
        gaps,
        trace: FreshnessTrace(trace),
    })
}

/// Refuses any input about another concept.
///
/// `P2-N2` found the same shape one layer up: a history that accepted another
/// concept's admitted evidence would have projected an `APPLIED` state out of
/// it, and none of its named tests would have caught it. Here the equivalent is
/// a `VERY_HIGH` band read off a neighbour's evidence with no edge cited at all.
fn require_about(concept: EntityId, inputs: FreshnessInputs<'_>) -> Result<(), FreshnessError> {
    if inputs.dated.iter().any(|item| item.concept() != concept)
        || inputs
            .statements
            .iter()
            .any(|statement| statement.concept() != concept)
        || inputs
            .contrary
            .iter()
            .any(|event| event.concept() != concept)
    {
        return Err(FreshnessError::EvidenceNamesAnotherConcept);
    }
    if inputs
        .spillover
        .iter()
        .any(|contribution| contribution.subject() != concept)
    {
        return Err(FreshnessError::SpilloverNamesAnotherConcept);
    }
    Ok(())
}

/// Refuses an input dated after the instant being asked about.
fn require_not_in_future(
    inputs: FreshnessInputs<'_>,
    as_of: TimestampMillis,
) -> Result<(), FreshnessError> {
    let future = inputs
        .dated
        .iter()
        .map(DatedEvidence::occurred_at)
        .chain(inputs.spillover.iter().map(Spillover::at))
        .chain(inputs.statements.iter().map(RecallStatement::stated_at))
        .chain(inputs.contrary.iter().map(ContraryEvent::observed_at))
        .any(|at| at.value() > as_of.value());
    if future {
        return Err(FreshnessError::InputAfterAsOf);
    }
    Ok(())
}

/// The most recent capping input, with the band it leaves reachable.
fn latest_cap(
    inputs: FreshnessInputs<'_>,
) -> Option<(FreshnessSignal, TimestampMillis, FreshnessBand)> {
    inputs
        .contrary
        .iter()
        .map(|event| {
            (
                FreshnessSignal::ContraryEvidence,
                event.observed_at(),
                event.kind().ceiling(),
            )
        })
        .chain(
            inputs
                .statements
                .iter()
                .filter(|statement| !statement.statement().raises())
                .map(|statement| {
                    (
                        FreshnessSignal::UserRecallStatement,
                        statement.stated_at(),
                        statement.statement().band(),
                    )
                }),
        )
        // Latest first, and the lower ceiling wins a tie so two events on one
        // day cannot cancel by ordering.
        .reduce(|held, next| {
            if next.1.value() > held.1.value()
                || (next.1.value() == held.1.value() && rank(next.2) < rank(held.2))
            {
                next
            } else {
                held
            }
        })
}

/// Elapsed milliseconds, refusing an input dated after `as_of`.
fn elapsed_or_refuse(
    earlier: TimestampMillis,
    as_of: TimestampMillis,
) -> Result<i64, FreshnessError> {
    crate::persistence::elapsed_millis(earlier, as_of).ok_or(FreshnessError::InputAfterAsOf)
}

/// A window extended by section 13.3's second input.
fn extended(window: PersistenceWindow, repeats: u32) -> PersistenceWindow {
    let days = window.days().saturating_mul(repeats.saturating_add(1));
    // `window` is already non-zero and the multiplier is at least one, so the
    // fallback is unreachable; it is the unextended window rather than a panic.
    PersistenceWindow::of_days(days).unwrap_or(window)
}

/// The later of an optional instant and one more.
fn later_of(held: Option<TimestampMillis>, next: TimestampMillis) -> TimestampMillis {
    match held {
        Some(held) if held.value() >= next.value() => held,
        _ => next,
    }
}
