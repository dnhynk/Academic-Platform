//! Evidence, and the separate thing an authority issues.
//!
//! # The two are different types on purpose
//!
//! Section 3.7: "`user_attestation` is an evidence kind, never a status
//! transition: a self-assessment cannot produce `PERMITTED`." Section 12.1 of
//! the authoritative spec says the same at length and adds the second case: a
//! self-judgement of the form "it is for personal use, so it must be fine"
//! does not move a permission to `PERMITTED` either.
//!
//! A run-time check would state that as a branch -- an `if kind ==
//! Attestation { deny }` somewhere -- and a branch is a thing that can be
//! deleted, reordered, or wrapped in a condition. So the separation here is the
//! type instead:
//!
//! * [`AttestationRecord`] is what a user files. It records *when* an oral
//!   permission was heard and the digest of the conditions heard with it, which
//!   is exactly what section 12.1 asks a user attestation to preserve.
//! * [`WrittenAuthority`] is what an instructor, an institution, or an
//!   accessibility determination issues, and it names a [`WrittenEvidenceKind`]
//!   that an attestation has no spelling in.
//! * [`AuthorityGrant::record`] takes the second and does not take the first.
//!
//! There is no `From`, no `TryFrom`, no `into_authority`, and no fallible
//! upgrade. Three separate things say so: the compiler, for a caller who tries
//! to pass one where the other is expected; the whole-set `impl` rule in
//! `consent_scans.rs`, for a conversion trait added later; and the
//! workspace-wide signature rule beside it, for a free function anywhere in
//! this repository that takes an attestation and returns an authority.
//!
//! # No evidence body is stored here
//!
//! Every evidence item is a locator plus a digest plus a byte count. No field
//! holds the syllabus text, the announcement body, or the words the user typed.
//! That is why this crate adds nothing to the `S-10` row in
//! `docs/contracts/policy-source-scans.md`: the generic secret-`Debug`
//! vocabulary reaches field names like `text` and `bytes`, and this crate
//! declares none. It is also why an evidence item needs no `Untrusted<T>`
//! wrapper from `P2-G5`: there are no ingested bytes here to mislabel. The
//! bytes stay in the vault the locator points at, and `P2-G5` owns them there.

use academic_domain::{ArtifactId, ContentDigest};

use crate::{
    ConsentError,
    permission::{CaptureMedium, CaptureProcessing, Condition, PermittedUse},
    retention::RetentionTerms,
};

/// A written form of evidence, by the kind of document it is.
///
/// Closed, and deliberately without an attestation spelling: a value of this
/// type is a claim that something outside the user's own account of events
/// exists to point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum WrittenEvidenceKind {
    /// The offering's syllabus.
    Syllabus,
    /// A course policy published in the learning management system.
    LmsPolicy,
    /// Correspondence from the granting authority.
    Correspondence,
    /// An announcement made to the whole offering.
    Announcement,
    /// A published institutional rule.
    InstitutionalRule,
    /// A determination issued by the accessibility office.
    AccessibilityDetermination,
}

impl WrittenEvidenceKind {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syllabus => "SYLLABUS",
            Self::LmsPolicy => "LMS_POLICY",
            Self::Correspondence => "CORRESPONDENCE",
            Self::Announcement => "ANNOUNCEMENT",
            Self::InstitutionalRule => "INSTITUTIONAL_RULE",
            Self::AccessibilityDetermination => "ACCESSIBILITY_DETERMINATION",
        }
    }
}

/// What a user files, and what it is never enough for.
///
/// Section 12.1 asks a user attestation to record when the oral permission was
/// heard and under what conditions, so both are required fields. What it does
/// not have is any route into [`AuthorityGrant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum AttestationKind {
    /// The user reports hearing the instructor permit recording.
    OralInstructorPermission,
    /// The user reports believing personal use makes the recording acceptable.
    PersonalUseBelief,
    /// The user reports a classmate's account of a permission.
    SecondHandReport,
}

impl AttestationKind {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OralInstructorPermission => "ORAL_INSTRUCTOR_PERMISSION",
            Self::PersonalUseBelief => "PERSONAL_USE_BELIEF",
            Self::SecondHandReport => "SECOND_HAND_REPORT",
        }
    }
}

/// An immutable thing a locator points at, referenced and never inlined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceArtifact {
    artifact_id: ArtifactId,
    digest: ContentDigest,
    byte_len: u64,
}

impl EvidenceArtifact {
    /// References an artifact by identity, digest, and length.
    #[must_use]
    pub const fn new(artifact_id: ArtifactId, digest: ContentDigest, byte_len: u64) -> Self {
        Self {
            artifact_id,
            digest,
            byte_len,
        }
    }

    /// The artifact this evidence points at.
    #[must_use]
    pub const fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    /// The digest of the bytes at that locator.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes that locator holds.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

/// A user's own account of events. Evidence, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationRecord {
    kind: AttestationKind,
    heard_at: u64,
    conditions_digest: ContentDigest,
}

impl AttestationRecord {
    /// Files an attestation.
    ///
    /// This returns an attestation and only an attestation. It is the whole
    /// public constructor surface of the type, and nothing on this type
    /// produces an [`AuthorityGrant`], a [`CaptureStatus`](crate::CaptureStatus),
    /// or a [`CaptureCapabilityToken`](crate::CaptureCapabilityToken).
    #[must_use]
    pub const fn file(
        kind: AttestationKind,
        heard_at: u64,
        conditions_digest: ContentDigest,
    ) -> Self {
        Self {
            kind,
            heard_at,
            conditions_digest,
        }
    }

    /// What kind of account this is.
    #[must_use]
    pub const fn kind(&self) -> AttestationKind {
        self.kind
    }

    /// When the user says the permission was heard.
    #[must_use]
    pub const fn heard_at(&self) -> u64 {
        self.heard_at
    }

    /// The digest of the conditions the user recorded hearing.
    #[must_use]
    pub const fn conditions_digest(&self) -> &ContentDigest {
        &self.conditions_digest
    }
}

/// Who granted. The section 3.7 set, which has no user on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum GrantAuthority {
    /// The instructor of the offering.
    Instructor,
    /// The institution, through a published rule.
    Institution,
    /// An accessibility accommodation determination.
    AccessibilityAccommodation,
}

impl GrantAuthority {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instructor => "INSTRUCTOR",
            Self::Institution => "INSTITUTION",
            Self::AccessibilityAccommodation => "ACCESSIBILITY_ACCOMMODATION",
        }
    }
}

/// A written act by one of the three section 3.7 authorities.
///
/// Both halves are required. An authority with no document to point at is not
/// representable, and neither is a document with no authority behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenAuthority {
    authority: GrantAuthority,
    kind: WrittenEvidenceKind,
    artifact: EvidenceArtifact,
}

impl WrittenAuthority {
    /// Records that an authority issued a written act.
    #[must_use]
    pub const fn new(
        authority: GrantAuthority,
        kind: WrittenEvidenceKind,
        artifact: EvidenceArtifact,
    ) -> Self {
        Self {
            authority,
            kind,
            artifact,
        }
    }

    /// Which of the three authorities acted.
    #[must_use]
    pub const fn authority(&self) -> GrantAuthority {
        self.authority
    }

    /// What kind of document records the act.
    #[must_use]
    pub const fn kind(&self) -> WrittenEvidenceKind {
        self.kind
    }

    /// The document itself.
    #[must_use]
    pub const fn artifact(&self) -> &EvidenceArtifact {
        &self.artifact
    }
}

/// A written authority's grant, with everything section 3.7 requires of one.
///
/// This is the only value in this crate from which a permitting status can be
/// derived, and [`AuthorityGrant::record`] is its only constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityGrant {
    authority: WrittenAuthority,
    permitted_use: PermittedUse,
    retention: RetentionTerms,
    conditions: Vec<Condition>,
    conditions_digest: ContentDigest,
    not_after: u64,
}

impl AuthorityGrant {
    /// Records a grant.
    ///
    /// The first parameter is a [`WrittenAuthority`]. An [`AttestationRecord`]
    /// is a different type with no conversion into it, so this signature is the
    /// whole of "a self-assessment cannot produce `PERMITTED`" -- there is no
    /// branch to delete and no flag to set.
    ///
    /// Every remaining parameter is required and none has a default. Section
    /// 3.7 gives `external_processing_allowed` and `sharing_allowed` a default
    /// of `0`, and they are inside [`PermittedUse`] as arguments a caller has
    /// to write rather than fields a caller can omit. There is no builder here
    /// for the same reason.
    #[must_use]
    pub fn record(
        authority: WrittenAuthority,
        permitted_use: PermittedUse,
        retention: RetentionTerms,
        conditions: Vec<Condition>,
        not_after: u64,
    ) -> Self {
        let mut listed = conditions;
        listed.sort_unstable();
        listed.dedup();
        let conditions_digest = conditions_digest(&listed);
        Self {
            authority,
            permitted_use,
            retention,
            conditions: listed,
            conditions_digest,
            not_after,
        }
    }

    /// The written act behind this grant.
    #[must_use]
    pub const fn authority(&self) -> &WrittenAuthority {
        &self.authority
    }

    /// Exactly what this grant covers.
    #[must_use]
    pub const fn permitted_use(&self) -> &PermittedUse {
        &self.permitted_use
    }

    /// The media this grant covers.
    #[must_use]
    pub fn allowed_media(&self) -> &[CaptureMedium] {
        self.permitted_use.allowed_media()
    }

    /// The processing this grant covers.
    #[must_use]
    pub fn allowed_processing(&self) -> &[CaptureProcessing] {
        self.permitted_use.allowed_processing()
    }

    /// Whether the grant reaches processing off this device.
    #[must_use]
    pub const fn external_processing_allowed(&self) -> bool {
        self.permitted_use.external_processing_allowed()
    }

    /// Whether the grant reaches sharing.
    #[must_use]
    pub const fn sharing_allowed(&self) -> bool {
        self.permitted_use.sharing_allowed()
    }

    /// The two independent retention bounds this grant carries.
    #[must_use]
    pub const fn retention(&self) -> &RetentionTerms {
        &self.retention
    }

    /// The conditions the grant attaches, sorted and deduplicated.
    #[must_use]
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    /// The section 3.7 `conditions_hash`.
    #[must_use]
    pub const fn conditions_digest(&self) -> &ContentDigest {
        &self.conditions_digest
    }

    /// When the grant stops covering anything.
    #[must_use]
    pub const fn not_after(&self) -> u64 {
        self.not_after
    }

    /// Checks a grant against the half-open interval it will be recorded in.
    pub(crate) const fn check_against(
        &self,
        valid_from: u64,
        valid_to: u64,
    ) -> Result<(), ConsentError> {
        if self.not_after > valid_to || self.not_after <= valid_from {
            return Err(ConsentError::GrantOutlivesScope);
        }
        Ok(())
    }
}

/// A written authority's refusal.
///
/// A refusal is written evidence too. Section 12.1 asks the same document trail
/// for "no" as for "yes", and a `PROHIBITED` a user typed with nothing behind
/// it would be a status transition from an attestation in the other direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalRecord {
    authority: WrittenAuthority,
    refused_at: u64,
}

impl RefusalRecord {
    /// Records a refusal.
    #[must_use]
    pub const fn record(authority: WrittenAuthority, refused_at: u64) -> Self {
        Self {
            authority,
            refused_at,
        }
    }

    /// The written act behind the refusal.
    #[must_use]
    pub const fn authority(&self) -> &WrittenAuthority {
        &self.authority
    }

    /// When the refusal was issued.
    #[must_use]
    pub const fn refused_at(&self) -> u64 {
        self.refused_at
    }
}

/// The section 3.7 `conditions_hash` over a sorted, deduplicated list.
fn conditions_digest(conditions: &[Condition]) -> ContentDigest {
    let mut material = b"academic-capture-conditions-v1\0".to_vec();
    push_len(&mut material, conditions.len());
    for condition in conditions {
        let spelling = condition.as_str().as_bytes();
        push_len(&mut material, spelling.len());
        material.extend_from_slice(spelling);
    }
    ContentDigest::sha256(&material)
}

/// Big-endian length prefix, the same shape `academic-policy` uses.
fn push_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&u64::try_from(len).unwrap_or(u64::MAX).to_be_bytes());
}
