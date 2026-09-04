//! The deterministic offering forecast, and the calibration it has to pass.
//!
//! # It is an engine-harness engine, and it is not one of the twelve
//!
//! `P2-C5` fixes every §28 engine as a pure
//! `(frozen_inputs, rule_set_hash, engine_version) -> (result, proof_tree,
//! explanation_snapshot)`, and this function is written to that signature: it
//! reads a clock nowhere, holds no RNG, opens no socket, calls no model, and
//! two evaluations over equal inputs under equal rule-set hashes produce
//! byte-equal [`academic_domain::engines::EngineOutcome::canonical_bytes`].
//!
//! **§28's table names twelve engines and none of them is an offering
//! forecast.** `schemas/registry/engine-registry-v1.json` is that table and
//! nothing else -- [the engine harness contract](../../../docs/contracts/engine-harness.md)
//! says the comparison against §28 is an enumeration, so an entry this task
//! added would fail `engine_registry_is_complete` against the design document.
//! So nothing here flips a registry entry, nothing here sits under
//! `testdata/engines/`, and [`OFFERING_FORECAST_ENGINE_ID`] is deliberately
//! outside that namespace. What is reused is the vocabulary and the discipline:
//! frozen inputs, a validated proof tree, a rendered explanation, a committed
//! golden corpus under `testdata/offering-forecast/`, and an independent oracle
//! in another language.
//!
//! # A prediction is never a confirmation
//!
//! There is no function in this module that returns a
//! [`crate::source::ConfirmationEvidence`], and there is no `From` between a
//! forecast and one. `ConfirmationEvidence` has private fields and one
//! constructor whose first argument is a registration-system reading, which a
//! forecast does not hold and cannot produce. That is the whole of the
//! `prediction_official_parallel` prohibition, and it is an absence rather than
//! a check.
//!
//! # Zero observations is an abstention, twice over
//!
//! [`AbstentionReason::NeverObserved`] is the explicit arm. Underneath it,
//! `academic_domain::PredictionMetadata::new` refuses a
//! `positive_sample_count` of zero, so a forecast over a never-observed course
//! has no metadata to disclose -- and [`ScoredForecast`] takes the metadata by
//! value, so there is no scored forecast for it to become. The check and the
//! type agree, and the type is the one that cannot be forgotten.

use std::collections::BTreeMap;

use academic_curriculum::CourseCode;
use academic_domain::{
    ConfidencePermille, ContentDigest, Decimal, PredictionMetadata, PredictionObservationWindow,
    TimestampMillis,
    engines::{
        EngineOutcome, EngineResult, EngineVersion, FrozenInputs, InputKey, InputValue, NodeId,
        ProofNode, ProofStatus, RuleId, RuleSetHash,
    },
};
use academic_model_run::{
    CalibratedConfidence, CalibrationRegistry, ModelVersion, ProviderId, Purpose, RawScore,
};
use academic_record::term::TermKey;

use crate::{
    error::OfferingError,
    feature::{FeatureFamily, FeatureVector, ObservationWindow},
    observation::{CourseHistory, Offered},
    policy::ForecastPolicy,
};

/// This engine's identifier.
///
/// Not a §28 registry `engine_id`: the twelve the registry holds are
/// `engine.*`, and an offering forecast is not one of them.
pub const OFFERING_FORECAST_ENGINE_ID: &str = "offering.forecast";

/// The version stamped into every outcome.
pub const OFFERING_FORECAST_ENGINE_VERSION: u16 = 1;

/// The provider identifier the calibration registry keys this forecaster on.
///
/// `P2-M1`'s registry is keyed by provider, model version and purpose because
/// two providers' raw numbers mean different things. A deterministic forecaster
/// is a producer of raw numbers like any other, and giving it its own key is
/// what stops its 620 being read through a dataset measured on somebody else's
/// scale.
pub const OFFERING_FORECAST_PROVIDER: &str = "snu.offering.history";

/// The purpose the calibration dataset must have been measured for.
pub const OFFERING_FORECAST_PURPOSE: &str = "offering.forecast.next_term";

/// The published rule set: every weight, every threshold, every arm.
///
/// Its SHA-256 is the `rule_set_hash` every outcome is bound to, so changing a
/// weight changes the canonical bytes of every evaluation. The text is the
/// contract a reader and the independent oracle both transcribe; it is not
/// rendered from the code, which is the point -- a rule set generated from the
/// implementation would agree with it by construction.
pub const FORECAST_RULE_SET: &str = "\
offering-forecast/1\n\
base=500\n\
clamp=0..1000\n\
window=seasonal, same semester as the forecast term, [from, to)\n\
seasonality: value=positive*1000/terms; contribution=(value-500)*2/5\n\
lifecycle_status: unknown=0/+0 established=1/+0 new_started=2/+60 \
new_not_yet=3/-500 sunset_after_target=4/-40 sunset_at_or_before_target=5/-500\n\
instructor_change: value=distinct instructor sets over offered terms; \
0/+0 1/+60 2/-60 3+/-120\n\
recent_notices: value=80*announced-200*suspended-60*curriculum_change; \
contribution=clamp(value,-300,300)\n\
offering_gap: value=seasonal terms since the last offered one; \
0/+60 1/-60 2/-160 3+/-260\n\
irregular_special: value=irregular offered terms; none=+0 all=-200 some=-100\n\
history_window: value=seasonal terms read; 0..1/-150 2/-40 3/+30 4+/+80\n\
abstain=never_observed, window_below_recorded_minimum, irregular_only, \
instructor_volatile, no_fresh_calibration_dataset\n\
likely=calibrated_permille>=recorded_floor\n";

/// The rule identifier every node of this engine's tree hangs from.
pub const RULE_OFFERING_FORECAST: &str = "offering.forecast.next_term";

/// Why the forecast declined to put a number on a course.
///
/// The first four are section 8.3's `UNCERTAIN` row read literally --
/// *표본 부족·불규칙·교수 변동* -- and [`Self::spec_phrase`] carries which of
/// the three each one is, compared against the document by
/// `the_abstention_reasons_are_section_8_3s_own`. The last three are the
/// last four are not grounds the row names: three are recorded criteria this
/// repository has no number for, and the fourth is section 8.3's
/// `HISTORICALLY_LIKELY` row losing its second conjunct. None of them carries a
/// phrase, because the specification writes none for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstentionReason {
    /// The window holds no term this course was observed running in.
    ///
    /// Section 8.3: *과거에 한 번도 관찰하지 못한 것은 `UNCERTAIN`이며 미개설
    /// 확정이 아니다.*
    NeverObserved,
    /// Fewer same-semester terms were read than the recorded minimum.
    WindowBelowRecordedMinimum,
    /// Every observed run was a one-off or special run.
    IrregularOnly,
    /// A different instructor set taught every observed run.
    InstructorVolatile,
    /// No forecast policy is recorded, so there is no floor to compare against.
    ForecastPolicyAbsent,
    /// No fresh calibration dataset interprets this forecaster's raw score.
    NoFreshCalibrationDataset,
    /// A calibrated probability below the recorded floor.
    BelowRecordedLikelyFloor,
    /// An official notice says the course will run and no listing has been
    /// verified, so the pattern no longer decides and the timetable is not
    /// known.
    AnnouncedButNotVerified,
}

impl AbstentionReason {
    /// Every reason.
    pub const ALL: [Self; 8] = [
        Self::NeverObserved,
        Self::WindowBelowRecordedMinimum,
        Self::IrregularOnly,
        Self::InstructorVolatile,
        Self::ForecastPolicyAbsent,
        Self::NoFreshCalibrationDataset,
        Self::BelowRecordedLikelyFloor,
        Self::AnnouncedButNotVerified,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeverObserved => "NEVER_OBSERVED",
            Self::WindowBelowRecordedMinimum => "WINDOW_BELOW_RECORDED_MINIMUM",
            Self::IrregularOnly => "IRREGULAR_ONLY",
            Self::InstructorVolatile => "INSTRUCTOR_VOLATILE",
            Self::ForecastPolicyAbsent => "FORECAST_POLICY_ABSENT",
            Self::NoFreshCalibrationDataset => "NO_FRESH_CALIBRATION_DATASET",
            Self::BelowRecordedLikelyFloor => "BELOW_RECORDED_LIKELY_FLOOR",
            Self::AnnouncedButNotVerified => "ANNOUNCED_BUT_NOT_VERIFIED",
        }
    }

    /// Which of section 8.3's three `UNCERTAIN` grounds this is, when it is one.
    #[must_use]
    pub const fn spec_phrase(self) -> Option<&'static str> {
        match self {
            Self::NeverObserved | Self::WindowBelowRecordedMinimum => Some("표본 부족"),
            Self::IrregularOnly => Some("불규칙"),
            Self::InstructorVolatile => Some("교수 변동"),
            Self::ForecastPolicyAbsent
            | Self::NoFreshCalibrationDataset
            | Self::BelowRecordedLikelyFloor
            | Self::AnnouncedButNotVerified => None,
        }
    }
}

/// A calibrated probability with the window that produced it.
///
/// Private fields, no public constructor, and [`forecast`] is the only site
/// that builds one. Both halves are required at once by construction: the
/// probability comes from `P2-M1`'s registry, which is the only producer of a
/// `CalibratedConfidence` in this workspace, and the window is
/// `academic_domain::PredictionMetadata`, whose constructor refuses a zero
/// positive-sample count. A scored forecast with an uncalibrated number or an
/// undisclosed window is therefore not a value that exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredForecast {
    calibrated: CalibratedConfidence,
    metadata: PredictionMetadata,
}

impl ScoredForecast {
    /// The calibrated probability, on `P2-M1`'s shared permille scale.
    #[must_use]
    pub const fn calibrated(&self) -> &CalibratedConfidence {
        &self.calibrated
    }

    /// The probability alone.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.calibrated.confidence()
    }

    /// The disclosed observation window and positive sample count.
    ///
    /// This is `§2.3-15`'s existing `prediction_metadata` shape at version 1,
    /// reused rather than replaced.
    #[must_use]
    pub const fn metadata(&self) -> PredictionMetadata {
        self.metadata
    }
}

/// What the forecast concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastVerdict {
    /// A calibrated probability over a disclosed window.
    Scored(ScoredForecast),
    /// The forecast declined, and named why.
    Abstained(AbstentionReason),
}

/// One course, one term, one evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forecast {
    course: CourseCode,
    target_term: TermKey,
    features: FeatureVector,
    raw_units: u32,
    verdict: ForecastVerdict,
    inputs: FrozenInputs,
    outcome: EngineOutcome,
}

impl Forecast {
    /// The course forecast.
    #[must_use]
    pub const fn course(&self) -> &CourseCode {
        &self.course
    }

    /// The term forecast.
    #[must_use]
    pub const fn target_term(&self) -> TermKey {
        self.target_term
    }

    /// Every family's reading.
    #[must_use]
    pub const fn features(&self) -> &FeatureVector {
        &self.features
    }

    /// The uncalibrated score every contribution summed to.
    ///
    /// Deliberately not a displayable number: nothing here formats it for a
    /// reader, and the only number a reader is shown comes through
    /// `academic_model_run::DisplayedConfidence`, which takes a calibrated
    /// value.
    #[must_use]
    pub const fn raw_units(&self) -> u32 {
        self.raw_units
    }

    /// What the forecast concluded.
    #[must_use]
    pub const fn verdict(&self) -> &ForecastVerdict {
        &self.verdict
    }

    /// The frozen inputs this evaluation read.
    #[must_use]
    pub const fn inputs(&self) -> &FrozenInputs {
        &self.inputs
    }

    /// The result, proof tree and rendered explanation.
    #[must_use]
    pub const fn outcome(&self) -> &EngineOutcome {
        &self.outcome
    }

    /// The canonical bytes two evaluations agree on when they agree.
    ///
    /// # Errors
    ///
    /// [`OfferingError`] when the engine version constant is out of range,
    /// which the workspace lints make a returned value rather than a panic.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OfferingError> {
        let version = EngineVersion::new(OFFERING_FORECAST_ENGINE_VERSION)
            .map_err(|error| OfferingError::Engine(error.to_string()))?;
        Ok(self.outcome.canonical_bytes(
            OFFERING_FORECAST_ENGINE_ID,
            rule_set_hash(),
            version,
            &self.inputs,
        ))
    }
}

/// The rule-set hash every evaluation is bound to.
#[must_use]
pub fn rule_set_hash() -> RuleSetHash {
    RuleSetHash::new(ContentDigest::sha256(FORECAST_RULE_SET.as_bytes()))
}

/// Evaluates one course's history against one term.
///
/// Pure: `now` is the caller's clock reading, taken as a value the way every
/// other engine in this workspace takes one, and nothing here reads a clock of
/// its own. The registry is consulted and never mutated.
///
/// # Errors
///
/// [`OfferingError`] when the window, the frozen inputs or the proof tree are
/// malformed. An absent calibration dataset, a short window and a
/// never-observed course are **not** errors: each is an [`AbstentionReason`]
/// on a forecast that exists and explains itself.
pub fn forecast(
    history: &CourseHistory,
    window: ObservationWindow,
    policy: ForecastPolicy,
    registry: &CalibrationRegistry,
    now: TimestampMillis,
) -> Result<Forecast, OfferingError> {
    let target_term = window.to();
    let features = FeatureVector::extract(history, window);
    let raw_units = features.raw_units();

    let verdict = decide(history, window, &features, raw_units, policy, registry, now)?;
    let root = root_status(&verdict, policy);
    let inputs = frozen_inputs(history, window, &features, raw_units, policy)?;
    let tree = proof_tree(&features, &verdict, policy, root)?;
    let result = engine_result(&features, raw_units, &verdict, root)?;
    let outcome = EngineOutcome::new(result, tree, &inputs)
        .map_err(|error| OfferingError::Engine(error.to_string()))?;

    Ok(Forecast {
        course: history.course().clone(),
        target_term,
        features,
        raw_units,
        verdict,
        inputs,
        outcome,
    })
}

/// How many observed runs it takes before every-run-a-different-instructor is
/// volatility rather than one change.
const INSTRUCTOR_VOLATILITY_RUNS: usize = 3;

fn decide(
    history: &CourseHistory,
    window: ObservationWindow,
    features: &FeatureVector,
    raw_units: u32,
    policy: ForecastPolicy,
    registry: &CalibrationRegistry,
    now: TimestampMillis,
) -> Result<ForecastVerdict, OfferingError> {
    if features.positive_samples() == 0 {
        return Ok(ForecastVerdict::Abstained(AbstentionReason::NeverObserved));
    }
    if features.seasonal_terms() < policy.minimum_window_terms() {
        return Ok(ForecastVerdict::Abstained(
            AbstentionReason::WindowBelowRecordedMinimum,
        ));
    }
    let seasonal = window.seasonal_terms(history);
    let offered: Vec<_> = seasonal
        .iter()
        .filter(|observation| observation.outcome() == Offered::Yes)
        .collect();
    if !offered.is_empty() && offered.iter().all(|observation| observation.is_irregular()) {
        return Ok(ForecastVerdict::Abstained(AbstentionReason::IrregularOnly));
    }
    let distinct_sets = features.signal(FeatureFamily::InstructorChange).value();
    if offered.len() >= INSTRUCTOR_VOLATILITY_RUNS
        && distinct_sets == i64::try_from(offered.len()).unwrap_or(i64::MAX)
    {
        return Ok(ForecastVerdict::Abstained(
            AbstentionReason::InstructorVolatile,
        ));
    }

    let provider = ProviderId::new(OFFERING_FORECAST_PROVIDER)?;
    let model_version = ModelVersion::new(OFFERING_FORECAST_ENGINE_VERSION.to_string())?;
    let purpose = Purpose::new(OFFERING_FORECAST_PURPOSE)?;
    let score = RawScore::new(provider, model_version, raw_units);
    let now_millis = u64::try_from(now.value()).unwrap_or(0);
    let Ok(calibrated) = registry.interpret(&score, &purpose, now_millis) else {
        return Ok(ForecastVerdict::Abstained(
            AbstentionReason::NoFreshCalibrationDataset,
        ));
    };
    let metadata =
        PredictionMetadata::new(disclosed_window(&seasonal)?, features.positive_samples())?;
    Ok(ForecastVerdict::Scored(ScoredForecast {
        calibrated,
        metadata,
    }))
}

/// The instant span the window's readings actually happened over.
///
/// `PredictionObservationWindow` is half-open and refuses `from >= to`, so a
/// window whose readings all landed on one instant is widened by one
/// millisecond rather than refused: one reading is still a bounded span, and
/// the positive-sample count beside it is what says how much evidence it is.
fn disclosed_window(
    seasonal: &[&crate::observation::TermObservation],
) -> Result<PredictionObservationWindow, OfferingError> {
    let mut earliest: Option<i64> = None;
    let mut latest: Option<i64> = None;
    for observation in seasonal {
        let read_at = observation.read_at().value();
        earliest = Some(earliest.map_or(read_at, |current: i64| current.min(read_at)));
        latest = Some(latest.map_or(read_at, |current: i64| current.max(read_at)));
    }
    let (Some(from), Some(to)) = (earliest, latest) else {
        return Err(OfferingError::EmptyWindow);
    };
    let to = to.checked_add(1).ok_or(OfferingError::EmptyWindow)?;
    Ok(PredictionObservationWindow::new(
        TimestampMillis::new(from),
        TimestampMillis::new(to),
    )?)
}

fn root_status(verdict: &ForecastVerdict, policy: ForecastPolicy) -> ProofStatus {
    match verdict {
        ForecastVerdict::Abstained(_) => ProofStatus::Unknown,
        ForecastVerdict::Scored(scored) => {
            if scored.confidence().value() >= policy.likely_floor_permille() {
                ProofStatus::Satisfied
            } else {
                ProofStatus::Needs
            }
        }
    }
}

fn key(name: &str) -> Result<InputKey, OfferingError> {
    InputKey::new(name).map_err(|error| OfferingError::Engine(error.to_string()))
}

fn node_id(name: &str) -> Result<NodeId, OfferingError> {
    NodeId::new(name).map_err(|error| OfferingError::Engine(error.to_string()))
}

fn rule_id(name: &str) -> Result<RuleId, OfferingError> {
    RuleId::new(name).map_err(|error| OfferingError::Engine(error.to_string()))
}

fn frozen_inputs(
    history: &CourseHistory,
    window: ObservationWindow,
    features: &FeatureVector,
    raw_units: u32,
    policy: ForecastPolicy,
) -> Result<FrozenInputs, OfferingError> {
    let mut entries: Vec<(InputKey, InputValue)> = Vec::new();
    for signal in features.signals() {
        entries.push((
            key(signal.family().input_key())?,
            InputValue::Integer(signal.value()),
        ));
    }
    entries.push((
        key("course.code")?,
        InputValue::Reference(history.course().as_str().to_owned()),
    ));
    entries.push((
        key("course.lifecycle")?,
        InputValue::Reference(history.lifecycle().as_str().to_owned()),
    ));
    entries.push((
        key("window.from")?,
        InputValue::Reference(window.from().canonical_text()),
    ));
    entries.push((
        key("window.to")?,
        InputValue::Reference(window.to().canonical_text()),
    ));
    entries.push((
        key("window.seasonal_terms")?,
        InputValue::Integer(i64::from(features.seasonal_terms())),
    ));
    entries.push((
        key("window.positive_samples")?,
        InputValue::Integer(i64::from(features.positive_samples())),
    ));
    entries.push((
        key("policy.likely_floor_permille")?,
        InputValue::Integer(i64::from(policy.likely_floor_permille())),
    ));
    entries.push((
        key("policy.minimum_window_terms")?,
        InputValue::Integer(i64::from(policy.minimum_window_terms())),
    ));
    entries.push((
        key("score.raw_units")?,
        InputValue::Integer(i64::from(raw_units)),
    ));
    FrozenInputs::new(entries).map_err(|error| OfferingError::Engine(error.to_string()))
}

/// The status one family's evidence carries.
///
/// `UNKNOWN` is a value, not a missing key: a family with nothing recorded to
/// read is *known to be unknown*, and folding it into a neutral `NEEDS` would
/// make a window nobody read look like a window that said nothing much.
fn family_status(contribution: i32, has_evidence: bool) -> ProofStatus {
    if !has_evidence {
        return ProofStatus::Unknown;
    }
    match contribution {
        0 => ProofStatus::Needs,
        value if value > 0 => ProofStatus::Satisfied,
        _ => ProofStatus::NotSatisfied,
    }
}

fn proof_tree(
    features: &FeatureVector,
    verdict: &ForecastVerdict,
    policy: ForecastPolicy,
    root: ProofStatus,
) -> Result<ProofNode, OfferingError> {
    let mut children = Vec::new();
    for (position, signal) in features.signals().iter().enumerate() {
        let family = signal.family();
        let has_evidence = if family == FeatureFamily::LifecycleStatus {
            signal.value() != 0
        } else {
            features.seasonal_terms() > 0
        };
        let suffix = lower(family.as_str());
        children.push(ProofNode {
            node_id: node_id(&format!("n.{position:02}.{suffix}"))?,
            rule_id: rule_id(&format!("{RULE_OFFERING_FORECAST}.{suffix}"))?,
            status: family_status(signal.contribution(), has_evidence),
            inputs: vec![key(family.input_key())?],
            source_locators: Vec::new(),
            children: Vec::new(),
        });
    }
    children.push(ProofNode {
        node_id: node_id("n.07.window")?,
        rule_id: rule_id(&format!("{RULE_OFFERING_FORECAST}.window"))?,
        status: if features.seasonal_terms() >= policy.minimum_window_terms() {
            ProofStatus::Satisfied
        } else {
            ProofStatus::NotSatisfied
        },
        inputs: vec![
            key("policy.minimum_window_terms")?,
            key("window.seasonal_terms")?,
        ],
        source_locators: Vec::new(),
        children: Vec::new(),
    });
    children.push(ProofNode {
        node_id: node_id("n.08.calibration")?,
        rule_id: rule_id(&format!("{RULE_OFFERING_FORECAST}.calibration"))?,
        status: match verdict {
            ForecastVerdict::Scored(_) => ProofStatus::Satisfied,
            ForecastVerdict::Abstained(_) => ProofStatus::Unknown,
        },
        inputs: vec![key("score.raw_units")?],
        source_locators: Vec::new(),
        children: Vec::new(),
    });

    Ok(ProofNode {
        node_id: node_id("n.root")?,
        rule_id: rule_id(RULE_OFFERING_FORECAST)?,
        status: root,
        inputs: vec![key("course.code")?, key("window.to")?],
        source_locators: Vec::new(),
        children,
    })
}

fn engine_result(
    features: &FeatureVector,
    raw_units: u32,
    verdict: &ForecastVerdict,
    root: ProofStatus,
) -> Result<EngineResult, OfferingError> {
    let mut values: BTreeMap<String, Decimal> = BTreeMap::new();
    for signal in features.signals() {
        values.insert(
            format!("contribution.{}", lower(signal.family().as_str())),
            whole(i128::from(signal.contribution()))?,
        );
    }
    values.insert("score.raw_units".to_owned(), whole(i128::from(raw_units))?);
    values.insert(
        "window.seasonal_terms".to_owned(),
        whole(i128::from(features.seasonal_terms()))?,
    );
    values.insert(
        "window.positive_samples".to_owned(),
        whole(i128::from(features.positive_samples()))?,
    );
    if let ForecastVerdict::Scored(scored) = verdict {
        values.insert(
            "forecast.calibrated_permille".to_owned(),
            whole(i128::from(scored.confidence().value()))?,
        );
    }
    Ok(EngineResult {
        status: root,
        values,
        unevaluated: Vec::new(),
    })
}

/// A whole number as an exact decimal.
///
/// Every number this crate produces goes through here. There is no `f32`, no
/// `f64` and no floating-point literal anywhere in the crate, and
/// `no_floating_point_reaches_a_forecast` is the whole-source statement of
/// that: a Brier score computed in binary floating point would be a
/// calibration metric that disagreed with itself across platforms.
fn whole(value: i128) -> Result<Decimal, OfferingError> {
    Ok(Decimal::new(value, 0)?)
}

fn lower(value: &str) -> String {
    value.to_ascii_lowercase()
}
