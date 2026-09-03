//! The structural and textual diff, and the rules it says are impacted.
//!
//! Two readings of the same document — an earlier snapshot's and a later one's
//! — are compared on both halves section 29.1 asks for. The *structural* half is
//! which section a rule sits in and whether it is present at all; the *textual*
//! half is the digest of the rule's text. Neither half carries document bytes:
//! a change is reported as an identifier, a section path, and two digests.
//!
//! # What "exact" means
//!
//! [`SourceDiff::impacted_rules`] names a rule when that rule changed, or when
//! the document's own header changed in a way that moves every rule in it — the
//! effective date, the target scope, the transitional measures, or the issuing
//! authority. It names no other rule.
//! `rule_change_impact_identifies_exact_rules` compares the whole set, so a
//! rule that did not change fails the assertion as an extra entry and a rule
//! that did fails it as a missing one.

use academic_domain::{ContentDigest, engines::RuleId};

use crate::{document::OfficialDocument, identifier::SectionPath};

/// One change to one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleChange {
    /// The rule is in the later reading and not the earlier one.
    Added {
        /// Which rule.
        id: RuleId,
        /// Where it appeared.
        section: SectionPath,
    },
    /// The rule is in the earlier reading and not the later one.
    Removed {
        /// Which rule.
        id: RuleId,
        /// Where it was.
        section: SectionPath,
    },
    /// The rule's text changed. The textual half.
    TextChanged {
        /// Which rule.
        id: RuleId,
        /// The digest before.
        before: ContentDigest,
        /// The digest after.
        after: ContentDigest,
    },
    /// The rule moved between sections. The structural half.
    Moved {
        /// Which rule.
        id: RuleId,
        /// Where it was.
        from: SectionPath,
        /// Where it is.
        to: SectionPath,
    },
}

impl RuleChange {
    /// Which rule this change is about.
    #[must_use]
    pub const fn rule(&self) -> &RuleId {
        match self {
            Self::Added { id, .. }
            | Self::Removed { id, .. }
            | Self::TextChanged { id, .. }
            | Self::Moved { id, .. } => id,
        }
    }
}

/// One change to the document's own header.
///
/// Each of these moves every rule the document carries, because each of them
/// decides when, to whom, and under whose authority those rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentChange {
    /// The issuing authority changed.
    Authority,
    /// The effective date changed, in either direction, including to or from
    /// `UNSCOPED_OFFICIAL_SOURCE`.
    EffectiveDate,
    /// The target scope changed.
    TargetScope,
    /// The transitional measures changed.
    TransitionalMeasures,
}

impl DocumentChange {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::Authority,
        Self::EffectiveDate,
        Self::TargetScope,
        Self::TransitionalMeasures,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authority => "AUTHORITY",
            Self::EffectiveDate => "EFFECTIVE_DATE",
            Self::TargetScope => "TARGET_SCOPE",
            Self::TransitionalMeasures => "TRANSITIONAL_MEASURES",
        }
    }
}

/// What changed between two readings of one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDiff {
    document_changes: Vec<DocumentChange>,
    rule_changes: Vec<RuleChange>,
    rules_after: Vec<RuleId>,
}

impl SourceDiff {
    /// Compares an earlier reading with a later one.
    #[must_use]
    pub fn between(previous: &OfficialDocument, current: &OfficialDocument) -> Self {
        let mut document_changes = Vec::new();
        if previous.authority() != current.authority() {
            document_changes.push(DocumentChange::Authority);
        }
        if previous.dating() != current.dating() {
            document_changes.push(DocumentChange::EffectiveDate);
        }
        if previous.scope() != current.scope() {
            document_changes.push(DocumentChange::TargetScope);
        }
        if previous.transitional_measures() != current.transitional_measures() {
            document_changes.push(DocumentChange::TransitionalMeasures);
        }

        let mut rule_changes = Vec::new();
        for later in current.rules() {
            match previous.rule(later.id()) {
                None => rule_changes.push(RuleChange::Added {
                    id: later.id().clone(),
                    section: later.section().clone(),
                }),
                Some(earlier) => {
                    if earlier.section() != later.section() {
                        rule_changes.push(RuleChange::Moved {
                            id: later.id().clone(),
                            from: earlier.section().clone(),
                            to: later.section().clone(),
                        });
                    }
                    if earlier.text_digest() != later.text_digest() {
                        rule_changes.push(RuleChange::TextChanged {
                            id: later.id().clone(),
                            before: *earlier.text_digest(),
                            after: *later.text_digest(),
                        });
                    }
                }
            }
        }
        for earlier in previous.rules() {
            if current.rule(earlier.id()).is_none() {
                rule_changes.push(RuleChange::Removed {
                    id: earlier.id().clone(),
                    section: earlier.section().clone(),
                });
            }
        }

        Self {
            document_changes,
            rule_changes,
            rules_after: current
                .rules()
                .iter()
                .map(|rule| rule.id().clone())
                .collect(),
        }
    }

    /// What changed about the document itself.
    #[must_use]
    pub fn document_changes(&self) -> &[DocumentChange] {
        &self.document_changes
    }

    /// What changed about individual rules.
    #[must_use]
    pub fn rule_changes(&self) -> &[RuleChange] {
        &self.rule_changes
    }

    /// Whether the two readings are the same document.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.document_changes.is_empty() && self.rule_changes.is_empty()
    }

    /// Exactly the rules this change moves, sorted and without repeats.
    ///
    /// A rule named by a rule-level change is here. Every rule the later
    /// reading carries is here when the document's own header changed, because
    /// a header change moves when and to whom each of them applies. Nothing
    /// else is.
    #[must_use]
    pub fn impacted_rules(&self) -> Vec<RuleId> {
        let mut impacted: Vec<RuleId> = if self.document_changes.is_empty() {
            self.rule_changes
                .iter()
                .map(|change| change.rule().clone())
                .collect()
        } else {
            self.rules_after
                .iter()
                .cloned()
                .chain(self.rule_changes.iter().map(|change| change.rule().clone()))
                .collect()
        };
        impacted.sort();
        impacted.dedup();
        impacted
    }
}
