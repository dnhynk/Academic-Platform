//! Section 8.3's four offering statuses, as four types.
//!
//! | 상태 | 요건 | UI 문구 | Planner 취급 |
//! |---|---|---|---|
//! | `CONFIRMED` | 해당 학기 공식 수강편람/수강신청 시스템에 존재하고 최근 확인 | 공식 개설 확인 · 확인일 | 실제 시간표·정원 사용 |
//! | `HISTORICALLY_LIKELY` | 여러 과거 학기의 재현 가능한 패턴, 미래 공식 공지 없음 | 과거 패턴상 가능성 | placeholder만, 졸업계획 확정에 사용 금지 |
//! | `UNCERTAIN` | 표본 부족·불규칙·교수 변동 | 근거 부족 | 경고와 대체 경로 요구 |
//! | `CANCELLED/WITHDRAWN` | 공식 폐강·변경 공지 | 공식 취소 | 선택 불가, 과거 scenario 보존 |
//!
//! **The table has four rows and the fourth row's name has a slash in it.**
//! `t068` section 2.3-4 writes the fourth status as `CANCELLED`, migration
//! `0014`'s `CHECK` admits `CANCELLED`, and `academic_curriculum::OfferingStatus`
//! declares four variants. So `CANCELLED/WITHDRAWN` is one status under two
//! spellings and not a fifth, and this crate reuses `P2-U1`'s enumeration
//! rather than declaring a second vocabulary.
//! `the_four_standings_are_section_8_3s_own` reads the table out of the design
//! document and compares the four rows against these four types in both
//! directions, so a fifth row appearing in the specification fails here rather
//! than being folded into the nearest type.
//!
//! # Four types, not four labels on one
//!
//! Each row is its own struct, each carries the evidence its 요건 cell names,
//! and each has private fields with no public constructor. What that buys is
//! the prohibition this whole task exists for:
//!
//! - [`ConfirmedStanding`] holds a [`ConfirmationEvidence`], whose one
//!   constructor takes a `SourceCategory::RegistrationSystem` reading inside
//!   the recorded verification bound. A forecast holds no such reading, so
//!   **there is no expression that turns a prediction into a confirmation.**
//!   Not a check that could be skipped -- an argument that cannot be supplied.
//! - [`ConfirmedSeat`] is produced by [`ConfirmedStanding::seat`] and by nothing
//!   else, and [`crate::plan::DeterminatePlan`] takes seats **by value**. So
//!   `HISTORICALLY_LIKELY` cannot enter a determinate plan because there is no
//!   seat for it to enter as.
//! - [`HistoricallyLikelyStanding`] holds a
//!   [`crate::forecast::ScoredForecast`] by value, which holds a
//!   `CalibratedConfidence` -- issued only by `P2-M1`'s registry -- and a
//!   `PredictionMetadata`, whose constructor refuses a zero positive-sample
//!   count. A likely standing with an uncalibrated number or an undisclosed
//!   window is not a value that exists.
//!
//! `P2-N2`'s *`AutomaticLevel` has no `Fluent`*, `P2-R5`'s *`AuthorshipMode`
//! has no review value* and `P2-U2`'s *two-attestation gate is a type* are the
//! same shape.

use academic_curriculum::{Capacity, CourseCode, Meeting, OfferingStatus};
use academic_domain::TimestampMillis;
use academic_model_run::{CalibratedConfidence, CalibrationRegistry, DisplayedConfidence};
use academic_record::term::TermKey;

use crate::{
    error::OfferingError,
    feature::ObservationWindow,
    forecast::{AbstentionReason, Forecast, ForecastVerdict, ScoredForecast, forecast},
    observation::CourseHistory,
    policy::ForecastPolicy,
    source::{CancellationNotice, ConfirmationEvidence, OfficialTermReading},
};

/// Section 8.3's `CONFIRMED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedStanding {
    evidence: ConfirmationEvidence,
}

impl ConfirmedStanding {
    /// Section 8.3's UI 문구 cell for this row, verbatim.
    pub const UI_COPY: &'static str = "공식 개설 확인 · 확인일";

    /// Section 8.3's Planner 취급 cell for this row, verbatim.
    pub const PLANNER_TREATMENT: &'static str = "실제 시간표·정원 사용";

    /// The registration-system reading and its cross sources.
    #[must_use]
    pub const fn evidence(&self) -> &ConfirmationEvidence {
        &self.evidence
    }

    /// The 확인일 the UI copy names.
    #[must_use]
    pub const fn verified_at(&self) -> TimestampMillis {
        self.evidence.verified_at()
    }

    /// The seat this offering contributes to a determinate plan.
    ///
    /// The only producer of a [`ConfirmedSeat`] in this workspace. It carries
    /// the real timetable and the real seat count, which is exactly what
    /// section 8.3's Planner 취급 cell for this row says a planner uses.
    #[must_use]
    pub fn seat(&self) -> ConfirmedSeat {
        let basis = self.evidence.basis();
        ConfirmedSeat {
            course: basis.course().clone(),
            term: basis.term(),
            verified_at: self.evidence.verified_at(),
            capacity: basis.announced_capacity(),
            meetings: basis.meetings().to_vec(),
        }
    }
}

/// One committed seat: a confirmed offering, its real timetable and its real
/// capacity.
///
/// Private fields and no public constructor. [`ConfirmedStanding::seat`] is the
/// one site that builds one, and it exists on no other standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedSeat {
    course: CourseCode,
    term: TermKey,
    verified_at: TimestampMillis,
    capacity: Option<Capacity>,
    meetings: Vec<Meeting>,
}

impl ConfirmedSeat {
    /// The course.
    #[must_use]
    pub const fn course(&self) -> &CourseCode {
        &self.course
    }

    /// The term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// When the offering was last verified.
    #[must_use]
    pub const fn verified_at(&self) -> TimestampMillis {
        self.verified_at
    }

    /// The announced seat count, when the listing printed one.
    #[must_use]
    pub const fn capacity(&self) -> Option<Capacity> {
        self.capacity
    }

    /// The real timetable.
    #[must_use]
    pub fn meetings(&self) -> &[Meeting] {
        &self.meetings
    }
}

/// Section 8.3's `HISTORICALLY_LIKELY`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricallyLikelyStanding {
    scored: ScoredForecast,
}

impl HistoricallyLikelyStanding {
    /// Section 8.3's UI 문구 cell for this row, verbatim.
    pub const UI_COPY: &'static str = "과거 패턴상 가능성";

    /// Section 8.3's Planner 취급 cell for this row, verbatim.
    pub const PLANNER_TREATMENT: &'static str = "placeholder만, 졸업계획 확정에 사용 금지";

    /// The calibrated probability and the window that produced it.
    #[must_use]
    pub const fn scored(&self) -> &ScoredForecast {
        &self.scored
    }

    /// The probability as a reader is shown it.
    ///
    /// `DisplayedConfidence::of` takes a `CalibratedConfidence`, so this method
    /// exists only because the standing already holds one. There is no
    /// corresponding method anywhere for a raw score.
    #[must_use]
    pub fn displayed(&self) -> DisplayedConfidence {
        DisplayedConfidence::of(self.scored.calibrated())
    }

    /// The calibrated value itself.
    #[must_use]
    pub const fn calibrated(&self) -> &CalibratedConfidence {
        self.scored.calibrated()
    }
}

/// Section 8.3's `UNCERTAIN`.
///
/// It carries the scored forecast when one exists. A probability under the
/// recorded floor is still a probability section 8.3 asks be recorded per
/// course, and dropping it would lose the number the next term's calibration is
/// measured against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncertainStanding {
    reason: AbstentionReason,
    scored: Option<ScoredForecast>,
}

impl UncertainStanding {
    /// Section 8.3's UI 문구 cell for this row, verbatim.
    pub const UI_COPY: &'static str = "근거 부족";

    /// Section 8.3's Planner 취급 cell for this row, verbatim.
    pub const PLANNER_TREATMENT: &'static str = "경고와 대체 경로 요구";

    /// Why the forecast declined.
    #[must_use]
    pub const fn reason(&self) -> AbstentionReason {
        self.reason
    }

    /// The scored forecast, when a probability was produced and fell short.
    #[must_use]
    pub const fn scored(&self) -> Option<&ScoredForecast> {
        self.scored.as_ref()
    }
}

/// Section 8.3's `CANCELLED/WITHDRAWN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelledStanding {
    notice: CancellationNotice,
}

impl CancelledStanding {
    /// Section 8.3's UI 문구 cell for this row, verbatim.
    pub const UI_COPY: &'static str = "공식 취소";

    /// Section 8.3's Planner 취급 cell for this row, verbatim.
    pub const PLANNER_TREATMENT: &'static str = "선택 불가, 과거 scenario 보존";

    /// The official cancellation or change notice.
    #[must_use]
    pub const fn notice(&self) -> &CancellationNotice {
        &self.notice
    }
}

/// Which of section 8.3's four rows an offering is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferingStanding {
    /// Present in the term's registration system and recently verified.
    Confirmed(ConfirmedStanding),
    /// A reproducible seasonal pattern, with no official future reading.
    HistoricallyLikely(HistoricallyLikelyStanding),
    /// The forecast declined, and says which of section 8.3's grounds it is on.
    Uncertain(UncertainStanding),
    /// An official cancellation or change notice exists.
    Cancelled(CancelledStanding),
}

impl OfferingStanding {
    /// `P2-U1`'s `OfferingStatus`, which is the value migration `0014`'s
    /// `CHECK` admits and the value section 8.2 puts on the aggregate.
    ///
    /// This crate declares no second vocabulary; it decides which of `P2-U1`'s
    /// four an offering carries.
    #[must_use]
    pub const fn status(&self) -> OfferingStatus {
        match self {
            Self::Confirmed(_) => OfferingStatus::Confirmed,
            Self::HistoricallyLikely(_) => OfferingStatus::HistoricallyLikely,
            Self::Uncertain(_) => OfferingStatus::Uncertain,
            Self::Cancelled(_) => OfferingStatus::Cancelled,
        }
    }

    /// Section 8.3's UI 문구 cell for this row.
    #[must_use]
    pub const fn ui_copy(&self) -> &'static str {
        match self {
            Self::Confirmed(_) => ConfirmedStanding::UI_COPY,
            Self::HistoricallyLikely(_) => HistoricallyLikelyStanding::UI_COPY,
            Self::Uncertain(_) => UncertainStanding::UI_COPY,
            Self::Cancelled(_) => CancelledStanding::UI_COPY,
        }
    }

    /// Section 8.3's Planner 취급 cell for this row.
    #[must_use]
    pub const fn planner_treatment(&self) -> &'static str {
        match self {
            Self::Confirmed(_) => ConfirmedStanding::PLANNER_TREATMENT,
            Self::HistoricallyLikely(_) => HistoricallyLikelyStanding::PLANNER_TREATMENT,
            Self::Uncertain(_) => UncertainStanding::PLANNER_TREATMENT,
            Self::Cancelled(_) => CancelledStanding::PLANNER_TREATMENT,
        }
    }

    /// The seat this standing contributes to a determinate plan.
    ///
    /// `Some` on exactly one row. The three other arms have no seat to return
    /// because [`ConfirmedSeat`] has one producer and it is on
    /// [`ConfirmedStanding`].
    #[must_use]
    pub fn seat(&self) -> Option<ConfirmedSeat> {
        match self {
            Self::Confirmed(confirmed) => Some(confirmed.seat()),
            Self::HistoricallyLikely(_) | Self::Uncertain(_) | Self::Cancelled(_) => None,
        }
    }
}

/// One offering's standing and the forecast that ran beside it.
///
/// Both, always. Section 30.1: *When A arrives, B is not rewritten as
/// official.* So an official reading decides [`Self::standing`] and the
/// forecast keeps its own probability, its own window and its own claim in
/// [`Self::forecast`]. `prediction_official_parallel` observes that the
/// forecast is byte-identical with and without the official reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    standing: OfferingStanding,
    forecast: Option<Forecast>,
}

impl Resolution {
    /// Which of section 8.3's four rows the offering is on.
    #[must_use]
    pub const fn standing(&self) -> &OfferingStanding {
        &self.standing
    }

    /// The forecast that ran, whether or not it decided the standing.
    ///
    /// `None` only when no forecast policy is recorded, because without a
    /// recorded floor there is nothing to compare a probability against.
    #[must_use]
    pub const fn forecast(&self) -> Option<&Forecast> {
        self.forecast.as_ref()
    }
}

/// Decides one offering's standing, and runs the forecast beside it.
///
/// # Errors
///
/// [`OfferingError`] when the forecast's inputs or proof tree are malformed.
/// An absent policy, an absent dataset and an absent history are not errors:
/// each produces an `UNCERTAIN` standing naming its own ground.
pub fn resolve(
    history: &CourseHistory,
    window: ObservationWindow,
    official: Option<&OfficialTermReading>,
    policy: Option<ForecastPolicy>,
    registry: &CalibrationRegistry,
    now: TimestampMillis,
) -> Result<Resolution, OfferingError> {
    // The forecast runs first and runs unconditionally, so an official reading
    // arriving cannot change what the prediction said. That order is the
    // section 30.1 parallel, written where it cannot be skipped.
    let prediction = match policy {
        Some(policy) => Some(forecast(history, window, policy, registry, now)?),
        None => None,
    };

    let standing = match official {
        Some(OfficialTermReading::Cancelled(notice)) => {
            OfferingStanding::Cancelled(CancelledStanding {
                notice: notice.clone(),
            })
        }
        Some(OfficialTermReading::Confirmed(evidence)) => {
            OfferingStanding::Confirmed(ConfirmedStanding {
                evidence: evidence.clone(),
            })
        }
        None => match (&prediction, policy) {
            (None, _) | (_, None) => OfferingStanding::Uncertain(UncertainStanding {
                reason: AbstentionReason::ForecastPolicyAbsent,
                scored: None,
            }),
            (Some(prediction), Some(policy)) => match prediction.verdict() {
                ForecastVerdict::Abstained(reason) => {
                    OfferingStanding::Uncertain(UncertainStanding {
                        reason: *reason,
                        scored: None,
                    })
                }
                ForecastVerdict::Scored(scored)
                    if scored.confidence().value() >= policy.likely_floor_permille() =>
                {
                    OfferingStanding::HistoricallyLikely(HistoricallyLikelyStanding {
                        scored: scored.clone(),
                    })
                }
                ForecastVerdict::Scored(scored) => OfferingStanding::Uncertain(UncertainStanding {
                    reason: AbstentionReason::BelowRecordedLikelyFloor,
                    scored: Some(scored.clone()),
                }),
            },
        },
    };

    Ok(Resolution {
        standing,
        forecast: prediction,
    })
}
