//! The deletion impact preview, extended from classes to the projections a
//! person actually reads.
//!
//! # `P2-G6`'s preview is called, not forked
//!
//! `academic_consent::preview_expiry` already walks every derivative class,
//! inherits each one's terms through the one inheritance function, and records
//! that the preview was shown. This module calls it and adds the layer section
//! 32.5 asks for on top: "어느 하나의 삭제가 concept/evidence projection에
//! 미치는 영향을 미리 보여준다". A second preview would be two answers to one
//! question, so [`preview_deletion`] produces one value carrying both.
//!
//! # The projection list is total, and the partition is what says so
//!
//! A preview that lists the projections somebody remembered is a preview with a
//! hole in it. So every deleted object is accounted for: it is cited by at
//! least one listed projection, or it is in
//! [`LectureDeletionPreview::unreferenced`], and the two sets are disjoint and
//! together are the deletion set. [`LectureDeletionPreview::partition_reconciles`]
//! states that, `deletion_impact_preview_lists_affected_projections` asserts it,
//! and an implementation that stopped walking the index would fail it rather
//! than report a shorter list.
//!
//! # An effect is what the projection loses, not whether it survives
//!
//! [`ProjectionEffect`] has two values: the projection loses every piece of
//! evidence it cites, or it loses some. Whether a projection with some evidence
//! left is still true is `P2-N2`'s question and is not answered here; what is
//! answered is which evidence goes.

use academic_consent::{
    ConsentLedger, DeletionImpact, ExpiryPlan, ExpiryRefusal, SubjectInventory, apply_expiry,
    preview_expiry,
};
use academic_domain::ContentDigest;

use crate::fault::DeletionFault;

/// The two projection families section 32.5 names.
///
/// Its sentence is "concept/evidence projection", and
/// `the_projection_families_are_section_32_5s_own` reads that phrase out of the
/// design document and compares this set in both directions. This is not
/// `academic_projections::ProjectionKind`, which names index kinds -- unicode,
/// trigram, graph -- and is a different question with a similar word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AffectedProjectionKind {
    /// What the user is taken to know about a concept.
    Concept,
    /// The evidence links a claim rests on.
    Evidence,
}

impl AffectedProjectionKind {
    /// Both families, in the order the specification names them.
    pub const ALL: [Self; 2] = [Self::Concept, Self::Evidence];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Concept => "CONCEPT",
            Self::Evidence => "EVIDENCE",
        }
    }

    /// The specification's own word for it.
    #[must_use]
    pub const fn spec_word(self) -> &'static str {
        match self {
            Self::Concept => "concept",
            Self::Evidence => "evidence",
        }
    }
}

/// One projection and the evidence it cites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRecord {
    kind: AffectedProjectionKind,
    id: String,
    cites: Vec<ContentDigest>,
}

impl ProjectionRecord {
    /// Records one projection.
    ///
    /// # Errors
    ///
    /// [`DeletionFault::ProjectionCitesNoEvidence`] for an empty citation list.
    /// A projection that cites nothing cannot be affected by a deletion, so
    /// admitting one would put a row in the index that no walk can ever reach
    /// -- the shape that makes a coverage number look complete while a case is
    /// unreachable.
    pub fn citing(
        kind: AffectedProjectionKind,
        id: &str,
        cites: Vec<ContentDigest>,
    ) -> Result<Self, DeletionFault> {
        if cites.is_empty() {
            return Err(DeletionFault::ProjectionCitesNoEvidence);
        }
        Ok(Self {
            kind,
            id: id.to_owned(),
            cites,
        })
    }

    /// Which family.
    #[must_use]
    pub const fn kind(&self) -> AffectedProjectionKind {
        self.kind
    }

    /// Its identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What it cites.
    #[must_use]
    pub fn cites(&self) -> &[ContentDigest] {
        &self.cites
    }
}

/// Which projections cite which objects.
///
/// This crate holds no projections. The index is what the layer that does
/// reports, and the preview is a pure function of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceIndex {
    projections: Vec<ProjectionRecord>,
}

impl EvidenceIndex {
    /// Builds an index.
    ///
    /// # Errors
    ///
    /// [`DeletionFault::ProjectionIsNamedTwice`] for a repeated identifier
    /// within one family: two rows with one identity would make the walk
    /// count one projection twice.
    pub fn of(projections: Vec<ProjectionRecord>) -> Result<Self, DeletionFault> {
        for (index, record) in projections.iter().enumerate() {
            if projections[..index]
                .iter()
                .any(|other| other.kind == record.kind && other.id == record.id)
            {
                return Err(DeletionFault::ProjectionIsNamedTwice);
            }
        }
        Ok(Self { projections })
    }

    /// Every projection, in the order it was recorded.
    #[must_use]
    pub fn projections(&self) -> &[ProjectionRecord] {
        &self.projections
    }
}

/// What a deletion does to one projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectionEffect {
    /// Every object it cites is going.
    LosesAllEvidence,
    /// Some are going and some remain.
    LosesSomeEvidence,
}

impl ProjectionEffect {
    /// Both effects.
    pub const ALL: [Self; 2] = [Self::LosesAllEvidence, Self::LosesSomeEvidence];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LosesAllEvidence => "LOSES_ALL_EVIDENCE",
            Self::LosesSomeEvidence => "LOSES_SOME_EVIDENCE",
        }
    }
}

/// One projection's line in the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedProjection {
    kind: AffectedProjectionKind,
    id: String,
    cited_total: usize,
    cited_deleted: usize,
    effect: ProjectionEffect,
}

impl AffectedProjection {
    /// Which family.
    #[must_use]
    pub const fn kind(&self) -> AffectedProjectionKind {
        self.kind
    }

    /// Which projection.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// How many objects it cites.
    #[must_use]
    pub const fn cited_total(&self) -> usize {
        self.cited_total
    }

    /// How many of them this deletion reaches.
    #[must_use]
    pub const fn cited_deleted(&self) -> usize {
        self.cited_deleted
    }

    /// What it loses.
    #[must_use]
    pub const fn effect(&self) -> ProjectionEffect {
        self.effect
    }
}

/// What a deletion at one instant would reach, in objects and in projections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LectureDeletionPreview {
    impact: DeletionImpact,
    deleted: Vec<ContentDigest>,
    projections: Vec<AffectedProjection>,
    unreferenced: Vec<ContentDigest>,
    digest: ContentDigest,
}

impl LectureDeletionPreview {
    /// `P2-G6`'s own preview, whole: both media lines and every derivative
    /// class in registry order.
    #[must_use]
    pub const fn impact(&self) -> &DeletionImpact {
        &self.impact
    }

    /// The objects this deletion reaches.
    #[must_use]
    pub fn deleted(&self) -> &[ContentDigest] {
        &self.deleted
    }

    /// Every projection that cites at least one of them, in index order.
    #[must_use]
    pub fn projections(&self) -> &[AffectedProjection] {
        &self.projections
    }

    /// The deleted objects no listed projection cites.
    ///
    /// A row rather than a hole. `P2-K5` made the same decision for a
    /// derivative class with nothing in it, and for the same reason: a class
    /// that vanishes from a report is indistinguishable from a class the walk
    /// never reached.
    #[must_use]
    pub fn unreferenced(&self) -> &[ContentDigest] {
        &self.unreferenced
    }

    /// The instant this preview describes, which is `P2-G6`'s.
    #[must_use]
    pub const fn previewed_at(&self) -> u64 {
        self.impact.previewed_at()
    }

    /// The digest a plan is compared against.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Whether every deleted object is either cited by a listed projection or
    /// listed as unreferenced, and never both.
    ///
    /// The totality statement. A walk that stopped short leaves an object in
    /// neither set; a walk that double-counts puts one in both.
    #[must_use]
    pub fn partition_reconciles(&self, index: &EvidenceIndex) -> bool {
        self.deleted.iter().all(|object| {
            let cited = index
                .projections
                .iter()
                .any(|record| record.cites.contains(object));
            let unreferenced = self.unreferenced.contains(object);
            cited != unreferenced
        })
    }

    /// The preview's canonical bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut text = format!(
            "academic-lecture-deletion-preview/1 {} {}\n",
            self.impact.digest(),
            self.impact.previewed_at()
        );
        for object in &self.deleted {
            text.push_str("deleted=");
            text.push_str(&object.to_string());
            text.push('\n');
        }
        for projection in &self.projections {
            text.push_str("projection=");
            text.push_str(projection.kind.as_str());
            text.push(' ');
            text.push_str(&projection.id);
            text.push(' ');
            text.push_str(&projection.cited_deleted.to_string());
            text.push('/');
            text.push_str(&projection.cited_total.to_string());
            text.push(' ');
            text.push_str(projection.effect.as_str());
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

/// Every projection that cites at least one deleted object, in index order.
///
/// The projection half of [`preview_deletion`], reachable on its own because
/// `P2-P2`'s artifact deletion asks section 32.5's question about an object
/// that is not always a lecture and so has no `P2-G6` expiry beneath it. It is
/// called by [`preview_deletion`] rather than copied there: two walks over one
/// index would be two answers to one question, which is the defect this module
/// was written to avoid one layer up.
#[must_use]
pub fn affected_projections(
    index: &EvidenceIndex,
    deleted: &[ContentDigest],
) -> Vec<AffectedProjection> {
    let mut projections = Vec::new();
    for record in &index.projections {
        let cited_deleted = record
            .cites
            .iter()
            .filter(|object| deleted.contains(object))
            .count();
        if cited_deleted == 0 {
            continue;
        }
        let effect = if cited_deleted == record.cites.len() {
            ProjectionEffect::LosesAllEvidence
        } else {
            ProjectionEffect::LosesSomeEvidence
        };
        projections.push(AffectedProjection {
            kind: record.kind,
            id: record.id.clone(),
            cited_total: record.cites.len(),
            cited_deleted,
            effect,
        });
    }
    projections
}

/// The deleted objects no projection in `index` cites, in deletion order.
///
/// The other half of the partition [`LectureDeletionPreview::partition_reconciles`]
/// states, extracted for the same reason as [`affected_projections`].
#[must_use]
pub fn unreferenced_objects(
    index: &EvidenceIndex,
    deleted: &[ContentDigest],
) -> Vec<ContentDigest> {
    deleted
        .iter()
        .filter(|object| {
            !index
                .projections
                .iter()
                .any(|record| record.cites.contains(object))
        })
        .copied()
        .collect()
}

/// Computes what a deletion at `at` would reach, in objects and in projections.
///
/// The object half is `academic_consent::preview_expiry`, called rather than
/// restated, so the ledger row that records "the preview was shown" is the same
/// row `P2-G6` writes and the derivative-class walk is the same walk.
#[must_use]
pub fn preview_deletion(
    ledger: &mut ConsentLedger,
    subject: &SubjectInventory,
    index: &EvidenceIndex,
    deleted: &[ContentDigest],
    at: u64,
) -> LectureDeletionPreview {
    let impact = preview_expiry(ledger, subject, at);
    let projections = affected_projections(index, deleted);
    let unreferenced = unreferenced_objects(index, deleted);
    let deleted = deleted.to_vec();
    let mut preview = LectureDeletionPreview {
        impact,
        deleted,
        projections,
        unreferenced,
        digest: ContentDigest::sha256(b""),
    };
    preview.digest = ContentDigest::sha256(&preview.canonical_bytes());
    preview
}

/// A deletion that has been previewed. It cannot be built any other way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LectureDeletionPlan {
    preview: LectureDeletionPreview,
}

impl LectureDeletionPlan {
    /// The only constructor: a plan is a preview somebody produced.
    #[must_use]
    pub const fn from_preview(preview: LectureDeletionPreview) -> Self {
        Self { preview }
    }

    /// What the user was shown.
    #[must_use]
    pub const fn preview(&self) -> &LectureDeletionPreview {
        &self.preview
    }
}

/// What an applied deletion reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeletionOutcome {
    objects_reached: u64,
    projections_affected: usize,
}

impl DeletionOutcome {
    /// How many objects `P2-G6`'s expiry reached.
    #[must_use]
    pub const fn objects_reached(&self) -> u64 {
        self.objects_reached
    }

    /// How many projections the preview listed.
    #[must_use]
    pub const fn projections_affected(&self) -> usize {
        self.projections_affected
    }
}

/// Applies a previewed deletion.
///
/// Three things are compared rather than trusted: the instant, which is
/// `apply_expiry`'s own check reached through it; the digest of the preview the
/// user was actually shown, which is this layer's; and the expiry itself, which
/// stays `P2-G6`'s.
///
/// # Errors
///
/// [`DeletionFault::PreviewDigestDoesNotMatch`] when `shown` is not this plan's
/// preview, [`DeletionFault::PreviewIsForAnotherInstant`] and
/// [`DeletionFault::NothingHasExpired`] from the expiry beneath it.
pub fn apply_deletion(
    ledger: &mut ConsentLedger,
    plan: &LectureDeletionPlan,
    shown: &ContentDigest,
    at: u64,
) -> Result<DeletionOutcome, DeletionFault> {
    if *shown != plan.preview.digest {
        return Err(DeletionFault::PreviewDigestDoesNotMatch);
    }
    let expiry = ExpiryPlan::from_preview(plan.preview.impact.clone());
    let objects_reached = apply_expiry(ledger, &expiry, at).map_err(|refusal| match refusal {
        ExpiryRefusal::PreviewIsForAnotherInstant => DeletionFault::PreviewIsForAnotherInstant,
        ExpiryRefusal::NothingHasExpired => DeletionFault::NothingHasExpired,
        _ => DeletionFault::ExpiryRefusedForAnUnknownReason,
    })?;
    Ok(DeletionOutcome {
        objects_reached,
        projections_affected: plan.preview.projections.len(),
    })
}
