//! Every refusal this crate can produce, in one closed set per boundary.
//!
//! The refusals are separated by boundary rather than merged into one enum so
//! that a caller matching on one cannot be handed a variant from another. That
//! is `P2-L4`'s split of `CoverageFault` from `DocumentFault` reused rather
//! than reinvented.

use academic_domain::ContentDigest;

/// Why a diarization corpus is not one.
///
/// A corpus that cannot measure the failure this task exists to bound is not a
/// weaker corpus: it is not evidence at all, so each of these is a refusal at
/// construction rather than a warning on a result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CorpusFault {
    /// A span ends at or before it starts.
    #[error("case {case}: {timeline} span {index} is empty or runs backwards")]
    EmptySpan {
        /// Which case.
        case: String,
        /// Which timeline: `reference` or `hypothesis`.
        timeline: &'static str,
        /// Which span of that timeline.
        index: usize,
    },
    /// Two spans of one timeline overlap, or they are out of order.
    #[error("case {case}: {timeline} span {index} overlaps or precedes the one before it")]
    OverlappingSpan {
        /// Which case.
        case: String,
        /// Which timeline.
        timeline: &'static str,
        /// Which span.
        index: usize,
    },
    /// A timeline has no spans.
    #[error("case {case}: the {timeline} timeline is empty")]
    EmptyTimeline {
        /// Which case.
        case: String,
        /// Which timeline.
        timeline: &'static str,
    },
    /// The hypothesis attributes time the reference does not cover.
    #[error("case {case}: the hypothesis runs past the reference")]
    HypothesisOutsideReference {
        /// Which case.
        case: String,
    },
    /// The reference names a speaker the ground truth cannot hold.
    ///
    /// A reference is what was actually said, so `unresolved` is not a value it
    /// can carry: an unlabelled ground truth would score every hypothesis as
    /// correct on that span.
    #[error("case {case}: the reference cannot leave a span unresolved")]
    UnresolvedInReference {
        /// Which case.
        case: String,
    },
    /// The corpus holds no student speech.
    ///
    /// The number this corpus exists to produce is how much student speech an
    /// automatic redaction would leave in. A corpus with no student speech
    /// divides that by zero, and reporting a perfect score for it would be the
    /// emptiest guard on this page.
    #[error("the corpus holds no student speech and cannot measure a missed redaction")]
    NoStudentSpeech,
    /// Two cases share a name.
    #[error("case {case} is named twice")]
    DuplicateCase {
        /// The repeated name.
        case: String,
    },
    /// The corpus has no cases.
    #[error("the corpus has no cases")]
    EmptyCorpus,
}

/// Why a measurement does not authorize an automatic redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AccuracyRefusal {
    /// The measured attribution accuracy is below the configured floor.
    #[error("measured accuracy {measured} permille is below the configured {required}")]
    AccuracyBelowThreshold {
        /// What the corpus measured.
        measured: u64,
        /// What the threshold requires.
        required: u64,
    },
    /// More student speech was labelled instructor than the threshold allows.
    ///
    /// This is the privacy failure and it is reported separately from accuracy
    /// on purpose: a corpus can score well overall while missing exactly the
    /// speech a redaction exists to remove.
    #[error(
        "{measured} permille of student speech was labelled instructor, above the allowed {allowed}"
    )]
    MissedStudentSpeechAboveThreshold {
        /// What the corpus measured.
        measured: u64,
        /// What the threshold allows.
        allowed: u64,
    },
}

/// Why a threshold is not one.
///
/// A threshold is configuration, and configuration that can be set to zero is
/// a guard a profile can delete. The band is not configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ThresholdFault {
    /// The accuracy floor a configuration may not go below.
    #[error("a required accuracy of {stated} permille is below the floor of {floor}")]
    AccuracyFloorIsBinding {
        /// What the caller asked for.
        stated: u64,
        /// The floor.
        floor: u64,
    },
    /// The missed-student ceiling a configuration may not go above.
    #[error(
        "an allowed missed-student fraction of {stated} permille is above the ceiling of {ceiling}"
    )]
    MissedStudentCeilingIsBinding {
        /// What the caller asked for.
        stated: u64,
        /// The ceiling.
        ceiling: u64,
    },
    /// A permille above one thousand.
    #[error("{stated} is not a permille")]
    AccuracyIsNotAPermille {
        /// What the caller asked for.
        stated: u64,
    },
}

/// Why a redaction was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RedactionFault {
    /// A manual plan named a segment the source does not have.
    #[error("segment {index} is not in the source")]
    NoSuchSegment {
        /// The index named.
        index: usize,
    },
    /// An automatic actor tried to decide a redaction.
    #[error("an automatic actor cannot decide a redaction")]
    AutomaticActorCannotRedact,
    /// The policy reference cites a policy other than the one supplied.
    #[error("the cited policy digest is not this policy's")]
    PolicyReferenceDoesNotResolve {
        /// What the reference cites.
        cited: ContentDigest,
        /// What the policy hashes to.
        actual: ContentDigest,
    },
    /// The source transcript has no version with this number.
    #[error("the transcript has no version {version}")]
    NoSuchVersion {
        /// The version asked for.
        version: u32,
    },
    /// A plan excluded nothing.
    ///
    /// An empty exclusion list is a redaction that redacts nothing, which would
    /// pass every check below by being vacuous.
    #[error("a redaction plan that excludes nothing is not a redaction")]
    NothingExcluded,
    /// The policy targets no speaker.
    #[error("a redaction policy has to target at least one speaker")]
    NoTargets,
    /// A manual exclusion named a segment the policy does not target.
    #[error("segment {index} is spoken by a speaker this policy does not target")]
    SegmentIsNotTargeted {
        /// The index named.
        index: usize,
    },
}

/// Why a restricted original refused an access.
///
/// There is deliberately no `GrantAlreadySpent` variant. A grant is consumed by
/// being moved into `RestrictedOriginal::open`, so spending one twice is a
/// program that does not compile rather than a refusal at run time -- and a
/// variant nothing can produce is a value this repository does not ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AccessRefusal {
    /// The grant names another original.
    #[error("this access grant was issued for another original")]
    GrantIsForAnotherOriginal,
    /// An automatic actor asked to read the original.
    #[error("an automatic actor cannot open a restricted original")]
    AutomaticActorCannotOpen,
}

/// Why a capture did not reach a downstream ingestion job.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum HoldRefusal {
    /// The capture is on hold and no review has released it.
    #[error("the capture is held pending review")]
    HeldPendingReview {
        /// Which classes were found, in registry order.
        classes: Vec<&'static str>,
    },
    /// A review was recorded and its outcome was to withhold.
    #[error("the review withheld this capture")]
    ReviewWithheld,
    /// The review does not account for every finding.
    #[error("the review leaves {count} finding(s) unaddressed")]
    ReviewIsIncomplete {
        /// How many findings the review did not name.
        count: usize,
    },
    /// An automatic actor tried to review a capture.
    #[error("an automatic actor cannot review a held capture")]
    AutomaticActorCannotReview,
    /// The review names a capture other than the one submitted.
    #[error("the review is for another capture")]
    ReviewIsForAnotherCapture,
}

/// Why a deletion preview or the plan over it was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DeletionFault {
    /// The plan describes another instant.
    #[error("the preview describes another instant")]
    PreviewIsForAnotherInstant,
    /// The plan's preview is not the one presented.
    #[error("the plan carries a preview other than the one shown")]
    PreviewDigestDoesNotMatch,
    /// A projection cites nothing.
    #[error("a projection that cites no evidence cannot be affected by a deletion")]
    ProjectionCitesNoEvidence,
    /// Two projections of one family share an identifier.
    #[error("a projection is named twice in one family")]
    ProjectionIsNamedTwice,
    /// Nothing would be deleted.
    #[error("nothing has reached its retention bound")]
    NothingHasExpired,
    /// `academic-consent` refused the expiry for a reason this crate was
    /// written before.
    ///
    /// `ExpiryRefusal` is `#[non_exhaustive]`, so the wildcard arm is required.
    /// It maps to a variant of its own rather than to
    /// [`DeletionFault::NothingHasExpired`], because reporting a refusal this
    /// crate does not understand as one it does is the class of error this
    /// whole page is about.
    #[error("the expiry beneath this deletion was refused for an unrecognised reason")]
    ExpiryRefusedForAnUnknownReason,
}
