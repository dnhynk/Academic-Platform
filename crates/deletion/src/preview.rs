//! The impact preview, and why it comes before the confirmation rather than
//! beside it.
//!
//! # Section 32.5's question, asked by `P2-L5`'s own walk
//!
//! *어느 하나의 삭제가 concept/evidence projection에 미치는 영향을 미리
//! 보여준다.* `P2-L5` answered that for a lecture expiry and this crate asks it
//! for an artifact deletion, so the walk is called rather than forked:
//! [`academic_student_voice::affected_projections`] and
//! [`academic_student_voice::unreferenced_objects`] are the same two functions
//! `preview_deletion` uses. A second walk over one index would be two answers
//! to one question.
//!
//! # The preview is total, and the citation map is where it could stop being
//!
//! Every artifact the dry run reaches is either cited by a listed projection or
//! listed as unreferenced, never both and never neither. That partition is only
//! meaningful if the preview knows the evidence key of every reached artifact,
//! so a citation map that is short is a refusal
//! ([`crate::DeletionFlowError::EvidenceCitationMissing`]) rather than a
//! shorter list. A preview that silently dropped an artifact it had no key for
//! would report fewer affected projections than the deletion reaches and would
//! look complete doing it.
//!
//! # It comes first because a confirmation cannot be built without one
//!
//! [`crate::DeletionConfirmation`] takes a [`DeletionImpactPreview`] by value
//! and there is no other constructor, so "preview precedes confirmation" is an
//! absent function rather than an ordering a caller has to remember.

use std::collections::BTreeMap;

use academic_domain::ContentDigest;
use academic_student_voice::{
    AffectedProjection, EvidenceIndex, affected_projections, unreferenced_objects,
};

use crate::{
    dry_run::DeletionDryRun, error::DeletionFlowError, protection::ProtectionDecision,
    target::DeletionTarget,
};

/// Which evidence digest each artifact is cited under.
///
/// This crate holds no store, so the mapping is an input. It has to be total
/// over the artifacts a dry run reaches; [`DeletionImpactPreview::of`] refuses
/// a map that is not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceCitations {
    entries: BTreeMap<DeletionTarget, ContentDigest>,
}

impl EvidenceCitations {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Records the digest one artifact is cited under.
    ///
    /// Keyed by the artifact *and* the locator, so two registrations of the
    /// same bytes are two entries. A map keyed by locator alone would give one
    /// of them the other's citations.
    pub fn cite(&mut self, target: DeletionTarget, digest: ContentDigest) {
        self.entries.insert(target, digest);
    }

    /// The digest one artifact is cited under, when the map holds it.
    #[must_use]
    pub fn digest_of(&self, target: &DeletionTarget) -> Option<ContentDigest> {
        self.entries.get(target).copied()
    }
}

/// What a deletion would do, in classes and in projections, before anyone
/// confirms it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionImpactPreview {
    dry_run: DeletionDryRun,
    reached: Vec<DeletionTarget>,
    deleted_evidence: Vec<ContentDigest>,
    projections: Vec<AffectedProjection>,
    unreferenced: Vec<ContentDigest>,
    previewed_at: u64,
    digest: ContentDigest,
}

impl DeletionImpactPreview {
    /// Computes the impact of one dry run at one instant.
    ///
    /// # Errors
    ///
    /// [`DeletionFlowError::Protected`] when a policy refuses the subject: a
    /// deletion that cannot run has no impact to preview, and returning one
    /// would put a confirmable value in the caller's hands.
    /// [`DeletionFlowError::EvidenceCitationMissing`] when `citations` does not
    /// cover every artifact the dry run reaches.
    pub fn of(
        dry_run: DeletionDryRun,
        index: &EvidenceIndex,
        citations: &EvidenceCitations,
        previewed_at: u64,
    ) -> Result<Self, DeletionFlowError> {
        if let ProtectionDecision::Protected(reason) = dry_run.protection() {
            return Err(DeletionFlowError::Protected {
                target: Box::new(*dry_run.subject()),
                reason: Box::new(reason.clone()),
            });
        }
        let reached = dry_run.reached();
        let mut deleted_evidence = Vec::with_capacity(reached.len());
        for target in &reached {
            let digest = citations
                .digest_of(target)
                .ok_or_else(|| DeletionFlowError::EvidenceCitationMissing(Box::new(*target)))?;
            deleted_evidence.push(digest);
        }
        let projections = affected_projections(index, &deleted_evidence);
        let unreferenced = unreferenced_objects(index, &deleted_evidence);
        let mut preview = Self {
            dry_run,
            reached,
            deleted_evidence,
            projections,
            unreferenced,
            previewed_at,
            digest: ContentDigest::sha256(b""),
        };
        preview.digest = ContentDigest::sha256(&preview.canonical_bytes());
        Ok(preview)
    }

    /// The dry run this preview is of, with its node per class.
    #[must_use]
    pub const fn dry_run(&self) -> &DeletionDryRun {
        &self.dry_run
    }

    /// Every artifact the deletion reaches, subject first.
    #[must_use]
    pub fn reached(&self) -> &[DeletionTarget] {
        &self.reached
    }

    /// Every projection that loses evidence, in index order.
    #[must_use]
    pub fn projections(&self) -> &[AffectedProjection] {
        &self.projections
    }

    /// The deleted evidence no listed projection cites.
    ///
    /// A row rather than a hole, for the reason `P2-K5` gives an empty
    /// derivative class one: a line that vanishes from a report cannot be told
    /// apart from a line the walk never reached.
    #[must_use]
    pub fn unreferenced(&self) -> &[ContentDigest] {
        &self.unreferenced
    }

    /// The instant this preview describes.
    #[must_use]
    pub const fn previewed_at(&self) -> u64 {
        self.previewed_at
    }

    /// The digest a confirmation is bound to.
    #[must_use]
    pub const fn digest(&self) -> ContentDigest {
        self.digest
    }

    /// Whether every reached artifact is cited by a listed projection or listed
    /// as unreferenced, and never both.
    #[must_use]
    pub fn partition_reconciles(&self, index: &EvidenceIndex) -> bool {
        self.deleted_evidence.iter().all(|object| {
            let cited = index
                .projections()
                .iter()
                .any(|record| record.cites().contains(object));
            let unreferenced = self.unreferenced.contains(object);
            cited != unreferenced
        })
    }

    /// The preview's canonical bytes.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut text = format!(
            "academic-deletion-impact-preview/1 {} {}\n",
            self.dry_run.subject().to_row(),
            self.previewed_at
        );
        for node in self.dry_run.nodes() {
            text.push_str("class=");
            text.push_str(node.class().as_str());
            text.push(' ');
            text.push_str(&node.targets().len().to_string());
            text.push('\n');
        }
        for target in &self.reached {
            text.push_str("reached=");
            text.push_str(&target.to_row());
            text.push('\n');
        }
        for projection in &self.projections {
            text.push_str("projection=");
            text.push_str(projection.kind().as_str());
            text.push(' ');
            text.push_str(projection.id());
            text.push(' ');
            text.push_str(&projection.cited_deleted().to_string());
            text.push('/');
            text.push_str(&projection.cited_total().to_string());
            text.push(' ');
            text.push_str(projection.effect().as_str());
            text.push('\n');
        }
        for object in &self.unreferenced {
            text.push_str("unreferenced=");
            text.push_str(&object.to_string());
            text.push('\n');
        }
        text.into_bytes()
    }
}
