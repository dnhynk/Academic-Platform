//! The deletion impact preview, and the plan that cannot exist without one.
//!
//! # Why the preview is a type and not a report
//!
//! This task's outcome requires "a preview of deletion impact before any expiry
//! action". A report that a caller may or may not have read is a convention; a
//! value the action requires is a rule. So [`ExpiryPlan::from_preview`] is the
//! only constructor of [`ExpiryPlan`], [`apply_expiry`] is the only consumer of
//! one, and there is no path from a subject to a deletion that does not pass
//! through a [`DeletionImpact`] first.
//!
//! [`apply_expiry`] also refuses a plan whose preview was taken at a different
//! instant from the one it is applied at. A preview is a statement about what
//! expires *now*; applying it later would delete against numbers the user never
//! saw.
//!
//! # Why the class list is declared here and not imported
//!
//! `P2-K5` fixed the closed list of things a deletion has to reach, in its
//! registry order, and made a class with nothing in it a node that says so
//! rather than a row that vanishes. That list is the right one and this module
//! walks it whole -- but importing it would mean a product edge to
//! `academic-retention`, and `rotation_engine_lane_is_not_default` in
//! `tools/phase1-scaffold-policy.test.mjs` holds that exactly one crate
//! declares that edge, because linking it links a crate that can destroy a key
//! slot. A consent ledger has no business inside that boundary.
//!
//! So the list is declared here, and the duplication is measured rather than
//! trusted: `academic-retention` is a **dev** dependency, and
//! `the_two_derivative_vocabularies_are_the_same_list` in `consent_scans.rs`
//! compares the two whole -- both orders and both sets of spellings -- so the
//! day either side gains, loses, or reorders a class, the suite fails. That is
//! the same shape `academic-untrusted-content` uses to keep `academic-policy`
//! off its product edge while still testing against a real broker.
//!
//! # Where the two independent bounds show up
//!
//! [`DeletionImpact`] has an audio node and a transcript node, each carrying
//! its own [`RetentionBound`] and its own verdict at the previewed instant, and
//! every derivative node carries the whole inherited
//! [`RetentionTerms`](crate::RetentionTerms) rather than one number. So a
//! subject whose audio has expired and whose transcript has not produces a
//! preview that says exactly that, and a model with one retention value could
//! not produce it.

use academic_domain::{CapturePermissionId, ContentDigest, OfferingId};

use crate::{
    ledger::{ConsentEventKind, ConsentLedger},
    permission::TermKey,
    retention::{RetentionBound, RetentionTerms},
};

/// One class of thing an expiry has to reach.
///
/// The list, its order, and its spellings are `academic-retention`'s
/// `DerivativeClass`, restated here because a product edge to that crate is
/// refused. `the_two_derivative_vocabularies_are_the_same_list` is what keeps
/// the restatement honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DerivativeClass {
    /// Lecture transcripts derived from the subject.
    Transcript,
    /// Vector embeddings computed over the subject or its transcript.
    Embedding,
    /// Canonical graph claims whose evidence is the subject.
    GraphClaim,
    /// Generated documents that quote or summarise the subject.
    Document,
    /// Local caches holding derived bytes.
    Cache,
    /// Replicas of the subject held elsewhere on this device.
    Replica,
    /// Backups whose expiry the deletion has to reach.
    BackupExpiry,
}

impl DerivativeClass {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transcript => "TRANSCRIPT",
            Self::Embedding => "EMBEDDING",
            Self::GraphClaim => "GRAPH_CLAIM",
            Self::Document => "DOCUMENT",
            Self::Cache => "CACHE",
            Self::Replica => "REPLICA",
            Self::BackupExpiry => "BACKUP_EXPIRY",
        }
    }
}

/// Every class, in the order a preview reports them.
pub const DERIVATIVE_CLASSES: &[DerivativeClass] = &[
    DerivativeClass::Transcript,
    DerivativeClass::Embedding,
    DerivativeClass::GraphClaim,
    DerivativeClass::Document,
    DerivativeClass::Cache,
    DerivativeClass::Replica,
    DerivativeClass::BackupExpiry,
];

/// What exists for one capture subject, as its owner reports it.
///
/// This crate holds no objects. The counts and the per-class requested terms
/// come from the caller that does, and the preview is a pure function of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectInventory {
    offering_id: OfferingId,
    term: TermKey,
    permission_id: CapturePermissionId,
    parent_terms: RetentionTerms,
    audio_objects: u64,
    transcript_objects: u64,
    derivatives: Vec<(DerivativeClass, u64, RetentionTerms)>,
}

impl SubjectInventory {
    /// Describes one subject.
    ///
    /// `derivatives` names only the classes that have something in them; the
    /// preview walks [`DERIVATIVE_CLASSES`] and reports the rest at zero, so a
    /// caller cannot hide a class by omitting it.
    #[must_use]
    pub fn new(
        offering_id: OfferingId,
        term: TermKey,
        permission_id: CapturePermissionId,
        parent_terms: RetentionTerms,
        audio_objects: u64,
        transcript_objects: u64,
        derivatives: Vec<(DerivativeClass, u64, RetentionTerms)>,
    ) -> Self {
        Self {
            offering_id,
            term,
            permission_id,
            parent_terms,
            audio_objects,
            transcript_objects,
            derivatives,
        }
    }

    /// The offering this subject belongs to.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The permission whose retention terms govern it.
    #[must_use]
    pub const fn permission_id(&self) -> CapturePermissionId {
        self.permission_id
    }
}

/// One medium's line in the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediumImpact {
    bound: RetentionBound,
    object_count: u64,
    expires_now: bool,
}

impl MediumImpact {
    /// The bound governing this medium.
    #[must_use]
    pub const fn bound(&self) -> RetentionBound {
        self.bound
    }

    /// How many objects the caller reported.
    #[must_use]
    pub const fn object_count(&self) -> u64 {
        self.object_count
    }

    /// Whether the bound has been reached at the previewed instant.
    #[must_use]
    pub const fn expires_now(&self) -> bool {
        self.expires_now
    }
}

/// One derivative class's line in the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivativeImpact {
    class: DerivativeClass,
    inherited: RetentionTerms,
    object_count: u64,
    audio_expires_now: bool,
    transcript_expires_now: bool,
}

impl DerivativeImpact {
    /// Which class.
    #[must_use]
    pub const fn class(&self) -> DerivativeClass {
        self.class
    }

    /// The terms this class inherited, both axes.
    #[must_use]
    pub const fn inherited(&self) -> RetentionTerms {
        self.inherited
    }

    /// How many objects the caller reported for it.
    #[must_use]
    pub const fn object_count(&self) -> u64 {
        self.object_count
    }

    /// Whether its audio axis has been reached.
    #[must_use]
    pub const fn audio_expires_now(&self) -> bool {
        self.audio_expires_now
    }

    /// Whether its transcript axis has been reached.
    #[must_use]
    pub const fn transcript_expires_now(&self) -> bool {
        self.transcript_expires_now
    }
}

/// What an expiry at one instant would reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionImpact {
    offering_id: OfferingId,
    term: TermKey,
    permission_id: CapturePermissionId,
    previewed_at: u64,
    audio: MediumImpact,
    transcript: MediumImpact,
    derivatives: Vec<DerivativeImpact>,
    digest: ContentDigest,
}

impl DeletionImpact {
    /// The offering.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The permission whose terms produced it.
    #[must_use]
    pub const fn permission_id(&self) -> CapturePermissionId {
        self.permission_id
    }

    /// The instant this preview describes.
    #[must_use]
    pub const fn previewed_at(&self) -> u64 {
        self.previewed_at
    }

    /// The audio line.
    #[must_use]
    pub const fn audio(&self) -> MediumImpact {
        self.audio
    }

    /// The transcript line.
    #[must_use]
    pub const fn transcript(&self) -> MediumImpact {
        self.transcript
    }

    /// One line per derivative class, in registry order, always.
    #[must_use]
    pub fn derivatives(&self) -> &[DerivativeImpact] {
        &self.derivatives
    }

    /// The digest an audit row names, over everything above.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many objects this expiry would reach in total.
    #[must_use]
    pub fn objects_reached(&self) -> u64 {
        let audio = if self.audio.expires_now {
            self.audio.object_count
        } else {
            0
        };
        let transcript = if self.transcript.expires_now {
            self.transcript.object_count
        } else {
            0
        };
        self.derivatives
            .iter()
            .filter(|node| node.audio_expires_now || node.transcript_expires_now)
            .map(|node| node.object_count)
            .fold(audio.saturating_add(transcript), u64::saturating_add)
    }
}

/// Computes what an expiry at `at` would reach, and records that it was shown.
///
/// Every derivative class is walked, and each inherits through
/// [`RetentionTerms::inherit`] rather than through a copy of the rule: there is
/// one inheritance function in this crate, and the preview a user reads is
/// produced by the same one the deletion later acts on.
pub fn preview_expiry(
    ledger: &mut ConsentLedger,
    subject: &SubjectInventory,
    at: u64,
) -> DeletionImpact {
    let parent = subject.parent_terms;
    let audio = MediumImpact {
        bound: parent.audio(),
        object_count: subject.audio_objects,
        expires_now: parent.audio().is_expired_at(at),
    };
    let transcript = MediumImpact {
        bound: parent.transcript(),
        object_count: subject.transcript_objects,
        expires_now: parent.transcript().is_expired_at(at),
    };
    let derivatives = DERIVATIVE_CLASSES
        .iter()
        .map(|class| {
            let reported = subject
                .derivatives
                .iter()
                .find(|(named, _, _)| named == class);
            let (object_count, requested) =
                reported.map_or((0, parent), |(_, count, requested)| (*count, *requested));
            let inherited = parent.inherit(requested);
            DerivativeImpact {
                class: *class,
                inherited,
                object_count,
                audio_expires_now: inherited.audio().is_expired_at(at),
                transcript_expires_now: inherited.transcript().is_expired_at(at),
            }
        })
        .collect::<Vec<_>>();
    let digest = impact_digest(subject, &audio, &transcript, &derivatives, at);
    ledger.record_expiry(
        ConsentEventKind::ExpiryPreviewed,
        subject.offering_id,
        subject.term,
        digest,
        at,
    );
    DeletionImpact {
        offering_id: subject.offering_id,
        term: subject.term,
        permission_id: subject.permission_id,
        previewed_at: at,
        audio,
        transcript,
        derivatives,
        digest,
    }
}

/// An expiry that has been previewed. It cannot be built any other way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiryPlan {
    impact: DeletionImpact,
}

impl ExpiryPlan {
    /// The only constructor: a plan is a preview somebody produced.
    #[must_use]
    pub fn from_preview(impact: DeletionImpact) -> Self {
        Self { impact }
    }

    /// What the user was shown.
    #[must_use]
    pub const fn impact(&self) -> &DeletionImpact {
        &self.impact
    }
}

/// Why an expiry was not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum ExpiryRefusal {
    /// The plan describes a different instant from the one it is applied at.
    #[error("the preview describes another instant")]
    PreviewIsForAnotherInstant,
    /// Nothing has reached its bound at this instant.
    #[error("nothing has reached its retention bound")]
    NothingHasExpired,
}

/// Applies a previewed expiry.
///
/// The instant is compared rather than trusted. A plan previewed at one instant
/// and applied at another would delete against a set the user was never shown,
/// which is the failure this whole module exists to make unreachable.
pub fn apply_expiry(
    ledger: &mut ConsentLedger,
    plan: &ExpiryPlan,
    at: u64,
) -> Result<u64, ExpiryRefusal> {
    if plan.impact.previewed_at != at {
        return Err(ExpiryRefusal::PreviewIsForAnotherInstant);
    }
    let reached = plan.impact.objects_reached();
    if reached == 0 {
        return Err(ExpiryRefusal::NothingHasExpired);
    }
    ledger.record_expiry(
        ConsentEventKind::ExpiryApplied,
        plan.impact.offering_id,
        plan.impact.term,
        plan.impact.digest,
        at,
    );
    Ok(reached)
}

/// The digest of one preview, over both axes and every class.
fn impact_digest(
    subject: &SubjectInventory,
    audio: &MediumImpact,
    transcript: &MediumImpact,
    derivatives: &[DerivativeImpact],
    at: u64,
) -> ContentDigest {
    let mut material = b"academic-deletion-impact-v1\0".to_vec();
    material.extend_from_slice(subject.permission_id.as_bytes());
    material.extend_from_slice(&at.to_be_bytes());
    push_medium(&mut material, audio);
    push_medium(&mut material, transcript);
    for node in derivatives {
        material.extend_from_slice(node.class.as_str().as_bytes());
        material.extend_from_slice(&node.object_count.to_be_bytes());
        push_bound(&mut material, node.inherited.audio());
        push_bound(&mut material, node.inherited.transcript());
    }
    ContentDigest::sha256(&material)
}

fn push_medium(material: &mut Vec<u8>, medium: &MediumImpact) {
    push_bound(material, medium.bound);
    material.extend_from_slice(&medium.object_count.to_be_bytes());
    material.push(u8::from(medium.expires_now));
}

fn push_bound(material: &mut Vec<u8>, bound: RetentionBound) {
    material.extend_from_slice(bound.kind_str().as_bytes());
    match bound {
        RetentionBound::Prohibited => material.extend_from_slice(&u64::MAX.to_be_bytes()),
        RetentionBound::Until(until) => material.extend_from_slice(&until.to_be_bytes()),
    }
}
