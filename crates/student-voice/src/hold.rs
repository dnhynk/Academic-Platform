//! The capture PII hold, and the two downstream jobs it stands in front of.
//!
//! # The hold is the absence of a method, not a flag
//!
//! Section 32.5: "Capture에 학생 얼굴·명단·개인 화면이 들어가면 review 전
//! graph/OCR ingestion을 보류." The way to get that wrong is a boolean nobody
//! downstream reads, which is the dominant defect class this Run has been
//! finding. So the hold is held three ways at once and the first is structural:
//!
//! * [`CaptureUnderReview`] holds the `CaptureBytes` privately and has **no
//!   byte accessor**. There is nothing to hand an OCR pass. That is
//!   `P2-L1`'s `QuarantinedArtifact` shape reused rather than reinvented.
//! * [`ReviewedCapture`] is the only type in this crate with a byte accessor,
//!   it has no public constructor, and its one producer is inside [`dispatch`].
//! * [`dispatch`] is the one door. It computes the hold state and calls the
//!   stage only on the arm that admits, so the guard has behavioural load:
//!   deleting the check makes a spy count a call, which is what
//!   `capture_pii_hold_blocks_downstream_jobs` measures.
//!
//! # The two jobs are section 32.5's own two
//!
//! [`IngestionJobKind`] has two variants because the specification's sentence
//! names two, and `the_downstream_jobs_are_section_32_5s_own` reads that
//! sentence out of the design document and compares the set in both
//! directions. A third job is a specification change, not a configuration one.
//!
//! # A review is a person's, and it has to address every finding
//!
//! [`ReviewDecision::recorded`] matches `academic-domain`'s closed `Actor`
//! exhaustively, and it refuses a decision that does not name every class the
//! findings hold. A reviewer who released a capture while looking at one of its
//! three findings would otherwise be a release nobody could tell from a
//! complete one.

use academic_capture::CaptureBytes;
use academic_domain::{Actor, ContentDigest};

use crate::fault::HoldRefusal;

/// What section 32.5 says must not reach an ingestion job unreviewed.
///
/// Three variants, which are the specification's own three: a student's face,
/// a roster, and somebody's personal screen. The list is closed and carries no
/// `#[non_exhaustive]`, so a fourth is a contract change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PiiClass {
    /// A student's face.
    StudentFace,
    /// A class roster or name list.
    Roster,
    /// Somebody's personal screen.
    PersonalScreen,
}

impl PiiClass {
    /// Every class, in the order a hold reports them.
    pub const ALL: [Self; 3] = [Self::StudentFace, Self::Roster, Self::PersonalScreen];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StudentFace => "STUDENT_FACE",
            Self::Roster => "ROSTER",
            Self::PersonalScreen => "PERSONAL_SCREEN",
        }
    }

    /// The specification's own phrase for it.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::StudentFace => "학생 얼굴",
            Self::Roster => "명단",
            Self::PersonalScreen => "개인 화면",
        }
    }
}

/// One thing a detector or a person found in a capture.
///
/// `detected_by` is any actor on purpose: a detector is a model and flagging is
/// exactly what a model may do. What a model may not do is decide the review,
/// and [`ReviewDecision::recorded`] is where that line is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiiFinding {
    class: PiiClass,
    detected_by: Actor,
}

impl PiiFinding {
    /// Records a finding.
    #[must_use]
    pub const fn found(class: PiiClass, detected_by: Actor) -> Self {
        Self { class, detected_by }
    }

    /// Which class.
    #[must_use]
    pub const fn class(&self) -> PiiClass {
        self.class
    }

    /// Who found it.
    #[must_use]
    pub const fn detected_by(&self) -> &Actor {
        &self.detected_by
    }
}

/// Whether a capture is held, and for what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldState {
    /// Nothing was found. The capture is not held.
    Clear,
    /// These classes were found, in registry order, each once.
    Held(Vec<PiiClass>),
}

impl HoldState {
    /// Whether this state blocks a downstream job.
    #[must_use]
    pub const fn is_held(&self) -> bool {
        matches!(self, Self::Held(_))
    }
}

/// What a review decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewOutcome {
    /// The capture may go downstream.
    Release,
    /// It may not.
    Withhold,
}

impl ReviewOutcome {
    /// Every outcome.
    pub const ALL: [Self; 2] = [Self::Release, Self::Withhold];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Release => "RELEASE",
            Self::Withhold => "WITHHOLD",
        }
    }
}

/// One completed review of one capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewDecision {
    capture_digest: ContentDigest,
    addressed: Vec<PiiClass>,
    outcome: ReviewOutcome,
    reviewed_by: Actor,
    at: u64,
}

impl ReviewDecision {
    /// Records a review.
    ///
    /// # Errors
    ///
    /// [`HoldRefusal::AutomaticActorCannotReview`] for every automatic actor,
    /// by an exhaustive `match` over `academic-domain`'s closed `Actor`.
    /// Whether a photograph of a lecture room may be processed is a judgement
    /// about other people's privacy and section 27.2 does not let a model make
    /// one.
    pub fn recorded(
        capture_digest: ContentDigest,
        addressed: Vec<PiiClass>,
        outcome: ReviewOutcome,
        reviewed_by: Actor,
        at: u64,
    ) -> Result<Self, HoldRefusal> {
        match &reviewed_by {
            Actor::User { .. } => Ok(Self {
                capture_digest,
                addressed,
                outcome,
                reviewed_by,
                at,
            }),
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                Err(HoldRefusal::AutomaticActorCannotReview)
            }
        }
    }

    /// Which capture.
    #[must_use]
    pub const fn capture_digest(&self) -> &ContentDigest {
        &self.capture_digest
    }

    /// Which classes the reviewer says they looked at.
    #[must_use]
    pub fn addressed(&self) -> &[PiiClass] {
        &self.addressed
    }

    /// What they decided.
    #[must_use]
    pub const fn outcome(&self) -> ReviewOutcome {
        self.outcome
    }

    /// Who they are.
    #[must_use]
    pub const fn reviewed_by(&self) -> &Actor {
        &self.reviewed_by
    }

    /// When.
    #[must_use]
    pub const fn at(&self) -> u64 {
        self.at
    }
}

/// A capture that has been screened and may or may not have been reviewed.
///
/// It holds the bytes and has no accessor for them. Everything a surface needs
/// in order to show that a capture exists and is held -- the digest, the size,
/// the classes -- is available; the content is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureUnderReview {
    digest: ContentDigest,
    bytes: CaptureBytes,
    findings: Vec<PiiFinding>,
    review: Option<ReviewDecision>,
}

impl CaptureUnderReview {
    /// Screens a capture.
    ///
    /// The findings are what a detector or a person reported. An empty list is
    /// a capture nothing was found in, which is [`HoldState::Clear`].
    #[must_use]
    pub fn screened(bytes: CaptureBytes, findings: Vec<PiiFinding>) -> Self {
        Self {
            digest: bytes.digest(),
            bytes,
            findings,
            review: None,
        }
    }

    /// The capture's identity.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes it is. Not what they are.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    /// What was found in it.
    #[must_use]
    pub fn findings(&self) -> &[PiiFinding] {
        &self.findings
    }

    /// The review, if one has been recorded.
    #[must_use]
    pub const fn review(&self) -> Option<&ReviewDecision> {
        self.review.as_ref()
    }

    /// Whether it is held, and for what.
    ///
    /// The classes are reported in [`PiiClass::ALL`] order and each at most
    /// once, so two findings of one class are one reason rather than two.
    #[must_use]
    pub fn hold_state(&self) -> HoldState {
        let classes: Vec<PiiClass> = PiiClass::ALL
            .into_iter()
            .filter(|class| self.findings.iter().any(|finding| finding.class == *class))
            .collect();
        if classes.is_empty() {
            HoldState::Clear
        } else {
            HoldState::Held(classes)
        }
    }

    /// Attaches a review.
    ///
    /// # Errors
    ///
    /// [`HoldRefusal::ReviewIsForAnotherCapture`] when the decision names a
    /// different capture, and [`HoldRefusal::ReviewIsIncomplete`] when it does
    /// not name every class the findings hold. A reviewer who saw one of three
    /// findings has not reviewed the capture.
    pub fn record_review(&mut self, decision: ReviewDecision) -> Result<(), HoldRefusal> {
        if decision.capture_digest != self.digest {
            return Err(HoldRefusal::ReviewIsForAnotherCapture);
        }
        let HoldState::Held(classes) = self.hold_state() else {
            self.review = Some(decision);
            return Ok(());
        };
        let unaddressed = classes
            .iter()
            .filter(|class| !decision.addressed.contains(class))
            .count();
        if unaddressed > 0 {
            return Err(HoldRefusal::ReviewIsIncomplete { count: unaddressed });
        }
        self.review = Some(decision);
        Ok(())
    }
}

/// What section 32.5 names as the two things a hold stands in front of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IngestionJobKind {
    /// Adding what the capture shows to the graph.
    GraphIngestion,
    /// Reading text out of the capture.
    OcrIngestion,
}

impl IngestionJobKind {
    /// Both jobs, in the order the specification names them.
    pub const ALL: [Self; 2] = [Self::GraphIngestion, Self::OcrIngestion];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GraphIngestion => "GRAPH_INGESTION",
            Self::OcrIngestion => "OCR_INGESTION",
        }
    }

    /// The specification's own word for it.
    #[must_use]
    pub const fn spec_word(self) -> &'static str {
        match self {
            Self::GraphIngestion => "graph",
            Self::OcrIngestion => "OCR",
        }
    }
}

/// A capture a downstream job may read.
///
/// The only type in this crate with a byte accessor. It has no public
/// constructor: the one place it is built is inside [`dispatch`], after the
/// hold state admitted, and `a_reviewed_capture_has_one_producer` counts that
/// site. Holding one is proof that the hold was passed rather than a claim that
/// it was.
#[derive(Debug, PartialEq, Eq)]
pub struct ReviewedCapture<'a> {
    digest: ContentDigest,
    kind: IngestionJobKind,
    bytes: &'a CaptureBytes,
}

impl ReviewedCapture<'_> {
    /// Which capture.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Which job it was admitted for.
    #[must_use]
    pub const fn kind(&self) -> IngestionJobKind {
        self.kind
    }

    /// The bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// One downstream ingestion job.
///
/// A trait the caller implements, the way `academic-ingestion` takes its
/// `ConditionalFetch`. This crate implements it nowhere: it holds no graph and
/// no OCR engine, and the acceptance suite's implementation is a counter.
pub trait IngestionStage {
    /// Reads one admitted capture.
    fn ingest(&mut self, capture: &ReviewedCapture<'_>);
}

/// What a dispatched job produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestionReceipt {
    digest: ContentDigest,
    kind: IngestionJobKind,
}

impl IngestionReceipt {
    /// Which capture.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Which job.
    #[must_use]
    pub const fn kind(&self) -> IngestionJobKind {
        self.kind
    }
}

/// Runs one downstream job over one capture, if the hold lets it.
///
/// The one door. A held capture with no review, or with a review that withheld,
/// returns a refusal and the stage is **not called**: the spy in
/// `capture_pii_hold_blocks_downstream_jobs` counts zero. A capture nothing was
/// found in needs no review, because there is nothing to review.
///
/// # Errors
///
/// [`HoldRefusal::HeldPendingReview`] and [`HoldRefusal::ReviewWithheld`].
pub fn dispatch<S: IngestionStage + ?Sized>(
    stage: &mut S,
    kind: IngestionJobKind,
    capture: &CaptureUnderReview,
) -> Result<IngestionReceipt, HoldRefusal> {
    if let HoldState::Held(classes) = capture.hold_state() {
        match capture.review.as_ref().map(|review| review.outcome) {
            None => {
                return Err(HoldRefusal::HeldPendingReview {
                    classes: classes.iter().map(|class| class.as_str()).collect(),
                });
            }
            Some(ReviewOutcome::Withhold) => return Err(HoldRefusal::ReviewWithheld),
            Some(ReviewOutcome::Release) => {}
        }
    }
    let admitted = ReviewedCapture {
        digest: capture.digest,
        kind,
        bytes: &capture.bytes,
    };
    stage.ingest(&admitted);
    Ok(IngestionReceipt {
        digest: capture.digest,
        kind,
    })
}
