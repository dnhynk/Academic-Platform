//! An official-source change, and the rules and plans it moves.
//!
//! Section 25.13's second bullet is *`official source change: 영향받는
//! rule/plan`*. Neither half is computed here: `P2-U6` owns both, and this
//! module names them.
//!
//! * the impacted rules are `SourceDiff::impacted_rules`, which reports a rule
//!   when that rule changed and every rule in the document when the document's
//!   own header changed;
//! * the impacted plans are `DependencyGraph::invalidate`, which walks the
//!   graph in reverse from those rules, transitively, and stops.
//!
//! Recomputing either here would give the center a second answer that could
//! disagree with the pipeline's. What this module adds is the *link*: one
//! entry that holds a change and both of its consequences, so a reader of the
//! centre sees what moved and what has to be redone together.
//!
//! # What a plan is
//!
//! Section 29.2's three dependent kinds are requirements, scenarios and course
//! mappings. Section 25.13's word is `plan`, and section 34.3's `공식 curriculum
//! 정보 변경` row says what a change does to one: *`영향 scenario/audit
//! invalidate`*. So a plan here is a `DependentNode` of any of the three kinds,
//! and [`SourceChangeEntry::impacted_plans`] returns all of them rather than
//! guessing which kind the word covers. [`SourceChangeEntry::plans_of_kind`] is
//! how a caller narrows it.

use academic_domain::{ContentDigest, TimestampMillis, engines::RuleId};
use academic_ingestion::{
    ConnectorId, DependencyGraph, DependentKind, DependentNode, DocumentChange, SourceDiff,
};

/// One official-source change, with the rules and plans it moves.
///
/// The two consequence lists are computed once, when the entry is built, from
/// the diff and the graph. They are stored rather than recomputed on each read
/// so that what the centre showed and what a caller acted on cannot differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChangeEntry {
    connector: ConnectorId,
    previous_content: ContentDigest,
    current_content: ContentDigest,
    observed_at: TimestampMillis,
    document_changes: Vec<DocumentChange>,
    impacted_rules: Vec<RuleId>,
    impacted_plans: Vec<DependentNode>,
}

impl SourceChangeEntry {
    /// Builds the entry from `P2-U6`'s own diff and graph.
    ///
    /// Nothing about which rules changed or which plans depend on them is
    /// decided here. The diff decides the first and the graph decides the
    /// second; this constructor records both against one change.
    #[must_use]
    pub fn from_diff(
        connector: ConnectorId,
        previous_content: ContentDigest,
        current_content: ContentDigest,
        observed_at: TimestampMillis,
        diff: &SourceDiff,
        graph: &DependencyGraph,
    ) -> Self {
        let impacted_rules = diff.impacted_rules();
        let impacted_plans = graph.invalidate(&impacted_rules).nodes().to_vec();
        Self {
            connector,
            previous_content,
            current_content,
            observed_at,
            document_changes: diff.document_changes().to_vec(),
            impacted_rules,
            impacted_plans,
        }
    }

    /// Which connector brought the change.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// The content digest of the reading this change is measured against.
    #[must_use]
    pub const fn previous_content(&self) -> ContentDigest {
        self.previous_content
    }

    /// The content digest of the reading that arrived.
    ///
    /// Two digests and no text. Section 29.1's textual diff is at rule
    /// granularity for the reason `P2-U6` records: a character diff would carry
    /// document bytes into every consumer of a change.
    #[must_use]
    pub const fn current_content(&self) -> ContentDigest {
        self.current_content
    }

    /// When the change was observed.
    #[must_use]
    pub const fn observed_at(&self) -> TimestampMillis {
        self.observed_at
    }

    /// Which of the document's own headers moved.
    ///
    /// Any of the four moves every rule in the document, because each decides
    /// when and to whom every rule applies.
    #[must_use]
    pub fn document_changes(&self) -> &[DocumentChange] {
        &self.document_changes
    }

    /// Exactly the rules `SourceDiff::impacted_rules` named.
    #[must_use]
    pub fn impacted_rules(&self) -> &[RuleId] {
        &self.impacted_rules
    }

    /// Exactly the plans `DependencyGraph::invalidate` reached.
    #[must_use]
    pub fn impacted_plans(&self) -> &[DependentNode] {
        &self.impacted_plans
    }

    /// The impacted plans of one dependent kind.
    #[must_use]
    pub fn plans_of_kind(&self, kind: DependentKind) -> Vec<&DependentNode> {
        self.impacted_plans
            .iter()
            .filter(|node| node.kind() == kind)
            .collect()
    }
}

/// Every observed official-source change.
#[derive(Debug, Clone, Default)]
pub struct SourceChangeLog {
    entries: Vec<SourceChangeEntry>,
}

impl SourceChangeLog {
    /// An empty log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Records one change.
    pub fn record(&mut self, entry: SourceChangeEntry) {
        self.entries.push(entry);
    }

    /// Every change, in observation order.
    #[must_use]
    pub fn entries(&self) -> &[SourceChangeEntry] {
        &self.entries
    }
}
