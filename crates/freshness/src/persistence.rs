//! Section 13.3's retention prior: what decays, how fast, and which half of
//! that is this task's answer and which half is `GATE-38-024`.
//!
//! ## Two persistence classes, and the direction between them is a measurement
//!
//! Section 13.3's third input reads
//! `노출·복습보다 실제 적용·debugging·설계에 더 긴 지속성`. That sentence names
//! two sides and orders them, and both are kept: [`PersistenceClass`] has one
//! variant per side, each carrying the document's own phrase, and
//! `debugging_evidence_persists_longer_than_exposure` splits the bullet on its
//! own `보다`, checks that the phrase before it is
//! [`PersistenceClass::ExposureOrReview`]'s and the phrase after it is
//! [`PersistenceClass::ApplicationOrDesign`]'s, and reads the relation word
//! `더 긴` out of the same line. **The direction is therefore read out of the
//! design document rather than out of the table below it.**
//!
//! ## What is a guess and what is not
//!
//! Two numbers live in [`UNCALIBRATED_PRIOR_V1`] and **neither is derived from
//! evidence**. `GATE-38-024` is exactly this: the evidence basis for the priors
//! and the speed of personalization are configuration decisions, and this task
//! does not make them. What is fixed here is everything around them:
//!
//! * the prior is **versioned** and named `UNCALIBRATED_PRIOR_V1`;
//! * its [`PriorBasis`] is `NO_EVIDENCE_BASIS_ESTABLISHED`, which is a value a
//!   caller reads rather than a comment;
//! * it is **visibly uncalibrated** — [`RetentionPrior::is_uncalibrated`] is
//!   true, [`crate::projection::FreshnessProjection`] carries a
//!   [`crate::projection::ConfidenceGap::PriorUncalibrated`] while it is, and
//!   the shipped default cannot be mistaken for a measured one;
//! * it stays **identifiable after calibration** — a calibrated prior keeps the
//!   identity it came from in [`RetentionPrior::origin`]; and
//! * the *speed* of personalization has **no default at all**.
//!   [`PersonalizationSpeed`] implements no `Default` and
//!   [`RetentionPrior::calibrate`] takes one by value, so nothing personalizes
//!   until somebody decides how fast. That is the half of `GATE-38-024` that is
//!   not merely labelled open but is unreachable without a decision.
//!
//! The one thing the shipped numbers are checked against is the design
//! document's own worked case: section 4's
//! `2년 전 배운 Virtual Memory는 mastery가 유지된 채 freshness가 STALE로 보일 수
//! 있고` and section 13.3's example block, where a lecture-taught concept with
//! `Recent use: none` reads `STALE`. `the_shipped_prior_does_not_contradict_the_document`
//! drives both. Passing that is not evidence that the numbers are right; it is
//! evidence that they are not already wrong.

use academic_domain::TimestampMillis;
use academic_knowledge_state::EvidenceKind;
use serde::{Deserialize, Serialize};

use crate::recall::{RecallCheck, RecallDirection};

/// Milliseconds in a day. Every window below is a whole number of days.
pub const DAY_MILLIS: i64 = 86_400_000;

/// Section 13.3's two sides of `노출·복습보다 실제 적용·debugging·설계`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PersistenceClass {
    /// `노출·복습` — the side the sentence says persists for less time.
    ExposureOrReview,
    /// `실제 적용·debugging·설계` — the side it says persists longer.
    ApplicationOrDesign,
}

impl PersistenceClass {
    /// Both, shorter-lived first.
    pub const ALL: [Self; 2] = [Self::ExposureOrReview, Self::ApplicationOrDesign];

    /// The design document's own phrase for this side, verbatim.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::ExposureOrReview => "노출·복습",
            Self::ApplicationOrDesign => "실제 적용·debugging·설계",
        }
    }
}

/// Which side of section 13.3's sentence a section 13.2 row falls on.
///
/// Total over `EvidenceKind` with no wildcard arm, so a ninth row added to
/// section 13.2 is a compile error here rather than a row that silently decays
/// at the exposure rate.
///
/// `CourseGrade` answers `None` and that is not an oversight. Section 13.2's
/// eighth row has no `ConceptEvidence` variant at all — `P2-N2` left nowhere to
/// write the concept down — so no dated evidence can ever carry that kind, and a
/// window for it would be a value nothing can reach.
/// `no_dated_evidence_can_carry_a_grade` observes the branch is empty over
/// every constructible value.
#[must_use]
pub const fn persistence_class(kind: EvidenceKind) -> Option<PersistenceClass> {
    match kind {
        // `transcript에서 meaningful teaching` is the `노출` of the sentence.
        EvidenceKind::MeaningfulTeaching
        // `사용자 자신의 설명 + 자기 확인` and `concept-specific 과제 풀이` are
        // rehearsal of an understanding rather than use of it in a real
        // context: the `복습` of the sentence.
        | EvidenceKind::SelfExplanationConfirmed
        | EvidenceKind::ConceptSpecificExercise
        // `dependency/install/import만 존재` is a technical contact that
        // promotes nothing; it is admitted, retained and shown, and it is the
        // weakest thing on the exposure side rather than a thing on the other.
        | EvidenceKind::DependencyPresenceOnly => Some(PersistenceClass::ExposureOrReview),
        // `직접 작성한 production/personal project code와 test` is `실제 적용`,
        // `incident debugging에서 원인 규명·수정·검증` is `debugging`, and
        // `서로 다른 맥락에서 반복 독립 수행·설계` is `설계`. Those are the
        // sentence's own three words, in its own order.
        EvidenceKind::AuthoredProjectCode
        | EvidenceKind::IncidentDebugging
        | EvidenceKind::RepeatedIndependentTransfer => Some(PersistenceClass::ApplicationOrDesign),
        EvidenceKind::CourseGrade => None,
    }
}

/// How long one class of evidence keeps a concept retrievable, in whole days.
///
/// Private field and one constructor, which rejects zero: a window of no length
/// would make every elapsed interval infinitely many windows and put every
/// concept in `STALE` regardless of its evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PersistenceWindow(u32);

impl PersistenceWindow {
    /// A window of `days`, or `None` for zero.
    #[must_use]
    pub const fn of_days(days: u32) -> Option<Self> {
        if days == 0 { None } else { Some(Self(days)) }
    }

    /// The window in days.
    #[must_use]
    pub const fn days(self) -> u32 {
        self.0
    }

    /// The window in milliseconds.
    #[must_use]
    pub const fn millis(self) -> i64 {
        self.0 as i64 * DAY_MILLIS
    }
}

/// The two names a retention prior can carry.
///
/// A closed vocabulary rather than a string, so a prior cannot be shipped under
/// a name that says it was calibrated when it was not: there are two values and
/// [`RetentionPrior::calibrate`] is the only thing that produces the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriorName {
    /// The shipped default. `GATE-38-024`.
    UncalibratedV1,
    /// Moved by the user's own recall record.
    UserCalibrated,
}

impl PriorName {
    /// Both.
    pub const ALL: [Self; 2] = [Self::UncalibratedV1, Self::UserCalibrated];

    /// The name the `P2-N3` contract fixes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UncalibratedV1 => "UNCALIBRATED_PRIOR_V1",
            Self::UserCalibrated => "USER_CALIBRATED",
        }
    }
}

/// The name and version of a retention prior.
///
/// A prior is versioned configuration, so its identity is a name and a
/// generation rather than a hash of its numbers: two calibrations that happen to
/// land on the same windows are still two generations, and a caller that pinned
/// one can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PriorIdentity {
    name: PriorName,
    generation: u32,
}

impl PriorIdentity {
    /// Names a generation.
    #[must_use]
    pub const fn of(name: PriorName, generation: u32) -> Self {
        Self { name, generation }
    }

    /// The prior's name.
    #[must_use]
    pub const fn name(self) -> PriorName {
        self.name
    }

    /// The prior's name, spelled.
    #[must_use]
    pub const fn name_str(self) -> &'static str {
        self.name.as_str()
    }

    /// Which generation of it.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// What the numbers in a prior rest on.
///
/// `GATE-38-024` is open, so the shipped value is the first variant and there is
/// no third: a prior either has an evidence basis somebody recorded, or it has
/// none and says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriorBasis {
    /// `GATE-38-024`: nobody has recorded an evidence basis for these numbers.
    NoEvidenceBasisEstablished,
    /// The user's own recall record, which is the only basis this system has.
    UserRecallRecord,
}

/// How fast the user's own recall record overrides the shipped prior.
///
/// **This is the half of `GATE-38-024` with no shipped value.** There is no
/// `Default`, no constant of this type in this crate, and
/// [`RetentionPrior::calibrate`] takes one by value, so nothing personalizes
/// until a configuration decision names a minimum sample count and a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PersonalizationSpeed {
    minimum_samples: u32,
    step_days: u32,
}

impl PersonalizationSpeed {
    /// Names both halves of the decision.
    ///
    /// Returns `None` for a zero minimum — calibrating on no sample at all is
    /// the cold-start case section 13.3 says uses the prior — and for a zero
    /// step, which would be a calibration that changes nothing while reporting
    /// that it happened.
    #[must_use]
    pub const fn of(minimum_samples: u32, step_days: u32) -> Option<Self> {
        if minimum_samples == 0 || step_days == 0 {
            return None;
        }
        Some(Self {
            minimum_samples,
            step_days,
        })
    }

    /// How many of the user's own recall checks are needed before anything
    /// moves.
    #[must_use]
    pub const fn minimum_samples(self) -> u32 {
        self.minimum_samples
    }

    /// How many days one net check moves a window.
    #[must_use]
    pub const fn step_days(self) -> u32 {
        self.step_days
    }
}

/// Whether a prior is still the shipped one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Calibration {
    /// Shipped, and not measured against this user.
    Uncalibrated,
    /// Moved by the user's own recall record, from the prior named here.
    Calibrated {
        /// The prior this one was calibrated from, which is what keeps the
        /// shipped default identifiable afterwards.
        origin: PriorIdentity,
        /// The configuration decision that set the speed.
        speed: PersonalizationSpeed,
        /// How many of the user's checks it rests on.
        samples: u32,
    },
}

/// Section 13.3's `concept별 retention profile과 사용자별 경험적 보정`.
///
/// One window per [`PersistenceClass`] and nothing finer, because section 13.3's
/// sentence licenses exactly one distinction and a finer split would be a number
/// nobody measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPrior {
    identity: PriorIdentity,
    basis: PriorBasis,
    calibration: Calibration,
    exposure: PersistenceWindow,
    application: PersistenceWindow,
}

/// The shipped default, and it says so in its own name.
///
/// `GATE-38-024`: 90 and 360 days are **not** an evidence-based estimate of
/// anything. They are versioned configuration carrying
/// [`PriorBasis::NoEvidenceBasisEstablished`], and the only properties this task
/// claims about them are that the second exceeds the first — which is section
/// 13.3's own sentence — and that neither contradicts the design document's two
/// worked cases.
pub const UNCALIBRATED_PRIOR_V1: RetentionPrior = RetentionPrior {
    identity: PriorIdentity::of(PriorName::UncalibratedV1, 1),
    basis: PriorBasis::NoEvidenceBasisEstablished,
    calibration: Calibration::Uncalibrated,
    exposure: PersistenceWindow(90),
    application: PersistenceWindow(360),
};

/// The name the shipped default carries, which is the name the `P2-N3`
/// contract fixes for it.
pub const UNCALIBRATED_PRIOR_NAME: &str = PriorName::UncalibratedV1.as_str();

/// The name a calibrated prior carries. Its origin is kept beside it.
pub const CALIBRATED_PRIOR_NAME: &str = PriorName::UserCalibrated.as_str();

impl RetentionPrior {
    /// This prior's own identity.
    #[must_use]
    pub const fn identity(&self) -> PriorIdentity {
        self.identity
    }

    /// What its numbers rest on.
    #[must_use]
    pub const fn basis(&self) -> PriorBasis {
        self.basis
    }

    /// Its calibration state.
    #[must_use]
    pub const fn calibration(&self) -> Calibration {
        self.calibration
    }

    /// Whether it is still the shipped default.
    ///
    /// A caller rendering a band reads this rather than comparing a name, which
    /// is what `GATE-38-024`'s *the shipped default must be visibly labelled
    /// uncalibrated* needs.
    #[must_use]
    pub const fn is_uncalibrated(&self) -> bool {
        matches!(self.calibration, Calibration::Uncalibrated)
    }

    /// The prior this one came from — itself when it was never calibrated.
    ///
    /// This is what keeps `UNCALIBRATED_PRIOR_V1` identifiable after
    /// calibration: a caller holding a calibrated prior can still name the
    /// shipped default it started from.
    #[must_use]
    pub const fn origin(&self) -> PriorIdentity {
        match self.calibration {
            Calibration::Uncalibrated => self.identity,
            Calibration::Calibrated { origin, .. } => origin,
        }
    }

    /// The window for one persistence class.
    #[must_use]
    pub const fn window_of(&self, class: PersistenceClass) -> PersistenceWindow {
        match class {
            PersistenceClass::ExposureOrReview => self.exposure,
            PersistenceClass::ApplicationOrDesign => self.application,
        }
    }

    /// The window for one section 13.2 row, or `None` for the grade row.
    #[must_use]
    pub const fn window_for(&self, kind: EvidenceKind) -> Option<PersistenceWindow> {
        match persistence_class(kind) {
            Some(class) => Some(self.window_of(class)),
            None => None,
        }
    }

    /// The shortest window this prior holds.
    #[must_use]
    pub const fn shortest(&self) -> PersistenceWindow {
        if self.exposure.days() <= self.application.days() {
            self.exposure
        } else {
            self.application
        }
    }

    /// Moves both windows by the user's own recall record.
    ///
    /// Section 13.3: `초기값은 prior이고 실제 사용자의 회상 확인으로
    /// calibration한다`. The record is the user's own checks and nothing else —
    /// there is no parameter here through which a projected band, a spillover
    /// contribution or another concept's state could reach the numbers, which is
    /// what keeps calibration from becoming a second propagation path.
    ///
    /// Below `speed.minimum_samples()` this returns the prior unchanged and
    /// still [`Calibration::Uncalibrated`], which is section 13.3's cold start.
    #[must_use]
    pub fn calibrate(&self, checks: &[RecallCheck], speed: PersonalizationSpeed) -> Self {
        let samples = u32::try_from(checks.len()).unwrap_or(u32::MAX);
        if samples < speed.minimum_samples() {
            return *self;
        }
        let mut net: i64 = 0;
        for check in checks {
            net += match check.direction() {
                RecallDirection::Retained => 1,
                RecallDirection::NotRetained => -1,
            };
        }
        let shift = net.saturating_mul(i64::from(speed.step_days()));
        Self {
            identity: PriorIdentity::of(
                PriorName::UserCalibrated,
                self.identity.generation().saturating_add(1),
            ),
            basis: PriorBasis::UserRecallRecord,
            calibration: Calibration::Calibrated {
                origin: self.origin(),
                speed,
                samples,
            },
            exposure: shifted(self.exposure, shift),
            application: shifted(self.application, shift),
        }
    }
}

/// Moves a window by `shift` days, never below one day.
fn shifted(window: PersistenceWindow, shift: i64) -> PersistenceWindow {
    let moved = i64::from(window.days()).saturating_add(shift).max(1);
    let days = u32::try_from(moved).unwrap_or(u32::MAX);
    // `of_days` refuses zero and `moved` is at least one, so the fallback is
    // unreachable; it is the argument rather than a panic.
    PersistenceWindow::of_days(days).unwrap_or(window)
}

/// Whole days between two instants, or `None` when `later` precedes `earlier`.
///
/// Freshness is elapsed time and a negative elapsed time is not a small one: an
/// input dated after the instant being asked about is a caller error, not a very
/// fresh input, so it has no answer here.
#[must_use]
pub fn elapsed_millis(earlier: TimestampMillis, later: TimestampMillis) -> Option<i64> {
    let span = later.value().checked_sub(earlier.value())?;
    if span < 0 { None } else { Some(span) }
}
