//! One consolidated inbox whose four proposal classes are four types.
//!
//! Section 25.13's first bullet is *`AI 제안 inbox: relation, concept merge,
//! project classification, state update`*. The four are distinguished by the
//! type of the payload each entry carries, not by a tag a caller writes beside
//! it:
//!
//! * there is no field anywhere in this module whose value is the class;
//! * [`ProposalClass`] has no `FromStr`, no `TryFrom<&str>` and no
//!   `From<&str>`, so no string produces one;
//! * [`InboxEntry::class`] is a total `match` over the four variants, so the
//!   class of an entry is read off its payload's type and cannot disagree with
//!   it;
//! * the four payload types share no field list, so one cannot be passed where
//!   another belongs.
//!   `tests/compile_fail/the_four_proposal_payloads_are_not_interchangeable.rs`
//!   is that as a compile error.
//!
//! A string tag would let two entries of one class disagree about what they
//! are, and would let a fifth class arrive without a type. Neither is
//! expressible here.

use academic_domain::{
    ConfidencePermille, EntityId, FindingId, MasteryLevel, ModelRunId, PredicateId, SnapshotId,
    TimestampMillis,
};
use academic_proposal::{ImpactPermille, ProposalId, RiskTier};

use crate::CenterError;

/// What every proposal carries whatever it proposes.
///
/// Section 29.7 batches a review queue on confidence *and* impact, and section
/// 27.4 decides how much of a human a change needs from its risk tier. All
/// three are `P2-M2`'s values, named here rather than restated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposalHeader {
    id: ProposalId,
    tier: RiskTier,
    confidence: ConfidencePermille,
    impact: ImpactPermille,
    model_run: ModelRunId,
    proposed_at: TimestampMillis,
}

impl ProposalHeader {
    /// The header of one proposal.
    #[must_use]
    pub const fn new(
        id: ProposalId,
        tier: RiskTier,
        confidence: ConfidencePermille,
        impact: ImpactPermille,
        model_run: ModelRunId,
        proposed_at: TimestampMillis,
    ) -> Self {
        Self {
            id,
            tier,
            confidence,
            impact,
            model_run,
            proposed_at,
        }
    }

    /// Which proposal.
    #[must_use]
    pub const fn id(&self) -> ProposalId {
        self.id
    }

    /// Section 27.4's tier.
    #[must_use]
    pub const fn tier(&self) -> RiskTier {
        self.tier
    }

    /// Section 29.7's confidence axis.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// Section 29.7's impact axis.
    #[must_use]
    pub const fn impact(&self) -> ImpactPermille {
        self.impact
    }

    /// Which model execution produced the candidate.
    #[must_use]
    pub const fn model_run(&self) -> ModelRunId {
        self.model_run
    }

    /// When the candidate was produced.
    #[must_use]
    pub const fn proposed_at(&self) -> TimestampMillis {
        self.proposed_at
    }
}

/// A relation between two entities, proposed by a model.
///
/// Section 34.2's `잘못된 prerequisite edge` row is what makes the predicate and
/// the corroboration count part of the payload rather than metadata: an edge is
/// wrong when its predicate is wrong, and a single-source hard edge is the shape
/// that row forbids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationProposal {
    header: ProposalHeader,
    subject: EntityId,
    predicate: PredicateId,
    object: EntityId,
    corroborating_sources: u32,
}

impl RelationProposal {
    /// A proposed relation.
    #[must_use]
    pub const fn new(
        header: ProposalHeader,
        subject: EntityId,
        predicate: PredicateId,
        object: EntityId,
        corroborating_sources: u32,
    ) -> Self {
        Self {
            header,
            subject,
            predicate,
            object,
            corroborating_sources,
        }
    }

    /// The proposal header.
    #[must_use]
    pub const fn header(&self) -> &ProposalHeader {
        &self.header
    }

    /// The relation's subject.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// The relation's predicate.
    #[must_use]
    pub const fn predicate(&self) -> &PredicateId {
        &self.predicate
    }

    /// The relation's object.
    #[must_use]
    pub const fn object(&self) -> EntityId {
        self.object
    }

    /// How many independent sources carry it.
    ///
    /// Section 30.5: several weak sources that copy one upstream source are not
    /// counted as independent corroboration. This crate records the count a
    /// caller computed; it does not compute one.
    #[must_use]
    pub const fn corroborating_sources(&self) -> u32 {
        self.corroborating_sources
    }
}

/// Two concept identities proposed as one.
///
/// Section 34.2's `synonym 중복` row: a merge is non-destructive and the
/// evidence count is shown *before* it, which is why the count is on the
/// proposal rather than on the outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConceptMergeProposal {
    header: ProposalHeader,
    retained: EntityId,
    absorbed: EntityId,
    evidence_before_merge: u32,
}

impl ConceptMergeProposal {
    /// A proposed merge.
    #[must_use]
    pub const fn new(
        header: ProposalHeader,
        retained: EntityId,
        absorbed: EntityId,
        evidence_before_merge: u32,
    ) -> Self {
        Self {
            header,
            retained,
            absorbed,
            evidence_before_merge,
        }
    }

    /// The proposal header.
    #[must_use]
    pub const fn header(&self) -> &ProposalHeader {
        &self.header
    }

    /// The identity that survives.
    #[must_use]
    pub const fn retained(&self) -> EntityId {
        self.retained
    }

    /// The identity that is absorbed.
    #[must_use]
    pub const fn absorbed(&self) -> EntityId {
        self.absorbed
    }

    /// The evidence count a reviewer sees before deciding.
    #[must_use]
    pub const fn evidence_before_merge(&self) -> u32 {
        self.evidence_before_merge
    }
}

/// How a repository finding is classified.
///
/// Section 34.4's ladder. The three tokens are that section's own and are named
/// here rather than re-derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FindingClassification {
    /// The artefact is present and nothing says it runs.
    PresentOnly,
    /// A path to it exists.
    Possible,
    /// It was observed running in this snapshot.
    Observed,
}

impl FindingClassification {
    /// Exhaustive listing, weakest first.
    pub const ALL: [Self; 3] = [Self::PresentOnly, Self::Possible, Self::Observed];
}

/// A repository finding proposed at some classification.
///
/// Section 34.4's `설치만 된 dependency를 사용으로 오인` row makes the snapshot part
/// of the payload: a classification is a claim about one immutable snapshot and
/// means nothing without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectClassificationProposal {
    header: ProposalHeader,
    project: EntityId,
    snapshot: SnapshotId,
    finding: FindingId,
    classification: FindingClassification,
}

impl ProjectClassificationProposal {
    /// A proposed classification.
    #[must_use]
    pub const fn new(
        header: ProposalHeader,
        project: EntityId,
        snapshot: SnapshotId,
        finding: FindingId,
        classification: FindingClassification,
    ) -> Self {
        Self {
            header,
            project,
            snapshot,
            finding,
            classification,
        }
    }

    /// The proposal header.
    #[must_use]
    pub const fn header(&self) -> &ProposalHeader {
        &self.header
    }

    /// Which project.
    #[must_use]
    pub const fn project(&self) -> EntityId {
        self.project
    }

    /// Which immutable snapshot the observation is scoped to.
    #[must_use]
    pub const fn snapshot(&self) -> SnapshotId {
        self.snapshot
    }

    /// Which finding.
    #[must_use]
    pub const fn finding(&self) -> FindingId {
        self.finding
    }

    /// The proposed rung of the ladder.
    #[must_use]
    pub const fn classification(&self) -> FindingClassification {
        self.classification
    }
}

/// A knowledge-state promotion or demotion proposed by a model.
///
/// Section 34.2's `state 과대승격` row is why both ends are on the payload: a
/// promotion is reviewable only when the reviewer can see what it moves *from*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateUpdateProposal {
    header: ProposalHeader,
    concept: EntityId,
    from_level: MasteryLevel,
    to_level: MasteryLevel,
}

impl StateUpdateProposal {
    /// A proposed state update.
    #[must_use]
    pub const fn new(
        header: ProposalHeader,
        concept: EntityId,
        from_level: MasteryLevel,
        to_level: MasteryLevel,
    ) -> Self {
        Self {
            header,
            concept,
            from_level,
            to_level,
        }
    }

    /// The proposal header.
    #[must_use]
    pub const fn header(&self) -> &ProposalHeader {
        &self.header
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The level the projection holds today.
    #[must_use]
    pub const fn from_level(&self) -> MasteryLevel {
        self.from_level
    }

    /// The level the proposal would move it to.
    #[must_use]
    pub const fn to_level(&self) -> MasteryLevel {
        self.to_level
    }
}

/// One entry in the consolidated inbox.
///
/// The payload's type *is* the class. There is no discriminant field, and no
/// constructor that takes a class beside a payload, so an entry cannot be
/// mislabelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxEntry {
    /// A proposed relation between two entities.
    Relation(RelationProposal),
    /// Two concept identities proposed as one.
    ConceptMerge(ConceptMergeProposal),
    /// A repository finding proposed at a classification.
    ProjectClassification(ProjectClassificationProposal),
    /// A knowledge-state move proposed by a model.
    StateUpdate(StateUpdateProposal),
}

/// The four classes, as a value.
///
/// This is a *label for* a class, produced from an entry. It is deliberately
/// not something an entry carries and deliberately not something a string
/// produces: `ProposalClass` implements no `FromStr`, no `TryFrom<&str>` and no
/// `From<&str>`, and `proposal_inbox_holds_four_typed_classes` reads the source
/// to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProposalClass {
    /// [`InboxEntry::Relation`].
    Relation,
    /// [`InboxEntry::ConceptMerge`].
    ConceptMerge,
    /// [`InboxEntry::ProjectClassification`].
    ProjectClassification,
    /// [`InboxEntry::StateUpdate`].
    StateUpdate,
}

impl ProposalClass {
    /// Exhaustive listing, in section 25.13's own reading order.
    pub const ALL: [Self; 4] = [
        Self::Relation,
        Self::ConceptMerge,
        Self::ProjectClassification,
        Self::StateUpdate,
    ];

    /// Section 25.13's own words for this class.
    ///
    /// Read *out of* the class, never into it.
    /// `the_six_sections_are_section_25_13s_own` removes these from the
    /// specification's first bullet and requires what remains to be
    /// punctuation.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::Relation => "relation",
            Self::ConceptMerge => "concept merge",
            Self::ProjectClassification => "project classification",
            Self::StateUpdate => "state update",
        }
    }
}

impl InboxEntry {
    /// The class of this entry, read off its payload's type.
    ///
    /// A total `match`, so a fifth variant stops this crate compiling until it
    /// names its class.
    #[must_use]
    pub const fn class(&self) -> ProposalClass {
        match self {
            Self::Relation(_) => ProposalClass::Relation,
            Self::ConceptMerge(_) => ProposalClass::ConceptMerge,
            Self::ProjectClassification(_) => ProposalClass::ProjectClassification,
            Self::StateUpdate(_) => ProposalClass::StateUpdate,
        }
    }

    /// The header every class carries.
    #[must_use]
    pub const fn header(&self) -> &ProposalHeader {
        match self {
            Self::Relation(payload) => payload.header(),
            Self::ConceptMerge(payload) => payload.header(),
            Self::ProjectClassification(payload) => payload.header(),
            Self::StateUpdate(payload) => payload.header(),
        }
    }
}

/// The one consolidated inbox.
///
/// It has no `remove`. Section 27's disposition history is `P2-M2`'s and is
/// append-only; an inbox that could drop an entry would be a second, silent
/// disposition.
#[derive(Debug, Clone, Default)]
pub struct ProposalInbox {
    entries: Vec<InboxEntry>,
}

impl ProposalInbox {
    /// An empty inbox.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Admits one entry.
    ///
    /// # Errors
    ///
    /// [`CenterError::ProposalAlreadyAdmitted`] when the identity is already
    /// present. Admission is not a disposition and records nothing.
    pub fn admit(&mut self, entry: InboxEntry) -> Result<(), CenterError> {
        let id = entry.header().id();
        if self
            .entries
            .iter()
            .any(|existing| existing.header().id() == id)
        {
            return Err(CenterError::ProposalAlreadyAdmitted { proposal: id });
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Every entry, in admission order.
    #[must_use]
    pub fn entries(&self) -> &[InboxEntry] {
        &self.entries
    }

    /// Exactly the entries of one class.
    ///
    /// The partition is by [`InboxEntry::class`], so it is by payload type.
    #[must_use]
    pub fn of_class(&self, class: ProposalClass) -> Vec<&InboxEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.class() == class)
            .collect()
    }
}
