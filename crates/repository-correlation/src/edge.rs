//! One typed evidence relation, the subject it is about, and what supports it.
//!
//! An edge is a value and never a row that is later corrected in place.
//! `CONTRIBUTING.md` rule 2 is that canonical claims are append-only and *a
//! correction is a new event plus an explicit relation or decision*; the same
//! rule applies here, and it is held by shape rather than by a check:
//! [`RelationEdge`] has private fields and one crate-private constructor, no
//! public function of this crate takes `&mut self`, and a conflict between two
//! lanes produces a [`crate::ImplementationDrift`] beside both edges rather
//! than an edit to either. `no_public_function_mutates_in_place` is the
//! executed half.

use academic_repository_analysis::{ArtifactScope, EvidenceTier, LadderRung, Locator};

use crate::{
    EvidenceRelation,
    artifact::{ApprovalStatus, DocumentId, IncidentId, IntentDocumentKind},
    relation::AuthorityLane,
};

/// What supports one edge.
///
/// Three arms because there are three kinds of producer, and they carry
/// different things: `P2-R2`'s ladder produced a rung and locators, a document
/// carried an approval status and a revision, and an incident carried a time.
/// Folding them into one arm with optional fields would make every reader
/// answer *which of these fields is meaningful here* at the use site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeEvidence {
    /// A `P2-R2` finding over this snapshot.
    Analysis {
        /// Which of section 17.3's five observations produced it.
        rung: LadderRung,
        /// The tier that rung folds onto.
        tier: EvidenceTier,
        /// Section 18.1's scope of the use.
        artifact_scope: ArtifactScope,
        /// Section 17.4's locators, carried through unchanged.
        locators: Vec<Locator>,
    },
    /// A specification, architecture decision or behaviour document.
    Document {
        /// Which document.
        document: DocumentId,
        /// Its approval status, or [`ApprovalStatus::Approved`] for a
        /// behaviour document, which describes rather than approves.
        status: ApprovalStatus,
        /// Section 30.3 row five's `최신`.
        revision: u64,
        /// Its path in the snapshot's manifest.
        path: String,
    },
    /// An incident record against this snapshot.
    Incident {
        /// Which incident.
        incident: IncidentId,
        /// When it happened.
        occurred_at: u64,
    },
}

/// One relation, from this snapshot, about one subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationEdge {
    relation: EvidenceRelation,
    subject: String,
    snapshot_id: String,
    evidence: EdgeEvidence,
}

impl RelationEdge {
    /// The one constructor, crate-private and called only from
    /// [`crate::correlate`]'s collector.
    pub(crate) const fn seal(
        relation: EvidenceRelation,
        subject: String,
        snapshot_id: String,
        evidence: EdgeEvidence,
    ) -> Self {
        Self {
            relation,
            subject,
            snapshot_id,
            evidence,
        }
    }

    /// Which of section 17.5's seven relations this is.
    #[must_use]
    pub const fn relation(&self) -> EvidenceRelation {
        self.relation
    }

    /// Which of section 30.3's rows it answers for.
    #[must_use]
    pub const fn lane(&self) -> AuthorityLane {
        self.relation.lane()
    }

    /// The caller's own identifier for what the edge is about.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Which snapshot the edge is about.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// What supports it.
    #[must_use]
    pub const fn evidence(&self) -> &EdgeEvidence {
        &self.evidence
    }

    /// The document kind this edge came from, when it came from a document.
    #[must_use]
    pub const fn document_kind(&self) -> Option<IntentDocumentKind> {
        match self.relation {
            EvidenceRelation::SpecMentions => Some(IntentDocumentKind::Specification),
            EvidenceRelation::ArchitectureRequires => {
                Some(IntentDocumentKind::ArchitectureDecision)
            }
            EvidenceRelation::CodeUses
            | EvidenceRelation::TestExercises
            | EvidenceRelation::ConfigEnables
            | EvidenceRelation::IncidentExposed
            | EvidenceRelation::DocExplains => None,
        }
    }
}
