//! `P2-U5`: section 8.3's four offering statuses, and the calibrated forecast
//! that decides which one an unconfirmed offering carries.
//!
//! This engine answers *will this course be offered next term*, and a user
//! plans their degree on the answer. So the contract behind every type here is
//! one sentence: **a prediction never becomes an official fact.**
//!
//! # The prohibition is an absence, not a check
//!
//! [`standing::ConfirmedStanding`] holds a [`source::ConfirmationEvidence`],
//! whose single constructor takes a `SourceCategory::RegistrationSystem`
//! reading inside a recorded verification bound and refuses every other level
//! of section 8.4. A forecast holds no such reading and there is no `From`, no
//! `promote`, and no `upgrade` anywhere in this crate. So the promotion section
//! 8.3 forbids is not a branch somebody could forget to write: it is an
//! argument that cannot be supplied. `P2-N2`'s *`AutomaticLevel` has no
//! `Fluent`*, `P2-R5`'s *`AuthorshipMode` has no review value*, `P2-L4`'s
//! *a `PdfArtifact` without a witness is `INCOMPLETE`* and `P2-U2`'s
//! *two-attestation gate is a type* are the same shape.
//!
//! The same absence carries the plan prohibition. Section 8.3's Planner 취급
//! cell for `HISTORICALLY_LIKELY` is *placeholder만, 졸업계획 확정에 사용 금지*,
//! and [`plan::DeterminatePlan::commit`] takes [`standing::ConfirmedSeat`]
//! values, whose one producer is `ConfirmedStanding::seat`. A likely standing
//! has no seat to hand it.
//!
//! # Seven feature families, and each one measured
//!
//! Section 8.3 refuses a majority vote and names the features instead.
//! [`feature::FeatureFamily`] is those, read out of the design document by
//! `the_feature_families_are_section_8_3s_own`, and
//! `offering_feature_contract` varies **one family at a time** and requires the
//! score to move each time -- so a family that was declared and never reached
//! the model fails rather than passing as documentation.
//!
//! # Zero observations abstains, twice
//!
//! Section 8.3: *과거에 한 번도 관찰하지 못한 것은 `UNCERTAIN`이며 미개설
//! 확정이 아니다.* [`forecast::AbstentionReason::NeverObserved`] is the explicit
//! arm, and `academic_domain::PredictionMetadata::new` refusing a zero
//! positive-sample count is the structural one: a never-observed course has no
//! metadata to disclose, and [`forecast::ScoredForecast`] takes the metadata by
//! value.
//!
//! # It reuses rather than rebuilds
//!
//! - `academic_curriculum::OfferingStatus` is the four-value vocabulary;
//!   this crate declares no second copy.
//! - `academic_domain::PredictionMetadata` at version 1 is the observation
//!   window, per `t068` section 2.3-15. Nothing new was minted for it.
//! - `academic_model_run::CalibrationRegistry` is the only producer of a
//!   displayable confidence, so a forecast with no fresh dataset abstains
//!   instead of showing an uncalibrated number.
//! - `academic_ingestion::SourceCategory` is section 8.4's six levels.
//! - `academic_record::TermKey` is the ordering, and
//!   `academic_record::PlanScenario` is the plan.
//!
//! # What this is not
//!
//! **It is not one of the twelve §28 engines.** The registry is the §28 table
//! and nothing else, and that table names no offering forecast. Nothing here
//! flips a registry entry and nothing here sits under `testdata/engines/`; what
//! is reused is the harness signature, the proof-tree vocabulary, a committed
//! golden corpus, and an independent oracle in another language.
//!
//! **It persists nothing.** There is no `academic-store` edge and no migration
//! number. Migration `0014` already holds `offering_detail.official_status`
//! with the four-value `CHECK`, and migration `0001` already holds
//! `prediction_metadata_version`; both are `P2-U1`'s and Phase 1's rows and
//! this crate writes neither.
//!
//! **It runs no connector and opens no socket.** Every reading arrives as a
//! value. Every fixture is synthetic and built by [`corpus`].
//!
//! **`GATE-38-017` stays open, every term.** See [`gate`].

pub mod claims;
pub mod corpus;
pub mod error;
pub mod feature;
pub mod forecast;
pub mod gate;
pub mod metrics;
pub mod observation;
pub mod plan;
pub mod policy;
pub mod source;
pub mod standing;

pub use claims::{
    ClaimSubject, DecisionStanding, OFFERING_STATUS_PREDICATE, OfferingAssertion, OfferingClaimSet,
    confirmation_claim, forecast_claim,
};
pub use error::OfferingError;
pub use feature::{
    BASE_RAW_UNITS, FeatureFamily, FeatureSignal, FeatureVector, MAX_RAW_UNITS, ObservationWindow,
};
pub use forecast::{
    AbstentionReason, FORECAST_RULE_SET, Forecast, ForecastVerdict, OFFERING_FORECAST_ENGINE_ID,
    OFFERING_FORECAST_ENGINE_VERSION, OFFERING_FORECAST_PROVIDER, OFFERING_FORECAST_PURPOSE,
    RULE_OFFERING_FORECAST, ScoredForecast, forecast, rule_set_hash,
};
pub use gate::OpenGate;
pub use metrics::{EvaluationEntry, RealizedOutcome, TermEvaluation, TermForecastMetrics};
pub use observation::{
    CourseHistory, CourseLifecycle, NoticeEffect, Offered, RecentNotice, TermObservation,
};
pub use plan::{DeterminatePlan, IndeterminatePlan, PlanOutcome, PlanRefusal};
pub use policy::{ForecastPolicy, VerificationRecency};
pub use source::{
    CancellationNotice, ConfirmationEvidence, CrossSourceDisagreement, OfficialListing,
    OfficialTermReading,
};
pub use standing::{
    CancelledStanding, ConfirmedSeat, ConfirmedStanding, HistoricallyLikelyStanding,
    OfferingStanding, Resolution, UncertainStanding, resolve,
};
