//! Stage nine, and the type that decides whether it can happen at all.
//!
//! Section 29.2: *a document whose effective date cannot be found is
//! `UNSCOPED_OFFICIAL_SOURCE` and is not automatically published as a rule.*
//!
//! [`publish`] takes a [`PublishableRules`]. That type has private fields, no
//! `Default`, and no public constructor;
//! [`crate::stage::Reconciled::publishable`] is its only producer and it
//! returns `None` for [`crate::dating::Dating::Unscoped`]. So an undated
//! document cannot be published — not because a check refuses it, but because
//! there is no value of the argument type to call [`publish`] with.
//! `tests/compile_fail/` observes both halves: the struct literal, and passing
//! the reconciled state where the publishable one belongs.
//!
//! A runtime check would sit one layer inside a function anybody can stop
//! calling. This does not.

use academic_domain::engines::RuleId;

use crate::{
    conflict::ConflictCase,
    dating::EffectiveDate,
    document::{OfficialDocument, TargetScope},
    identifier::ConnectorId,
    manifest::{ParserVersion, RetrievalInstant},
};

/// A document that states when it applies, together with what it publishes.
///
/// Private fields, no public constructor, and a lifetime tied to the reconciled
/// state it was read from, so it cannot outlive the run that produced it.
#[derive(Debug)]
pub struct PublishableRules<'run> {
    document: &'run OfficialDocument,
    connector: &'run ConnectorId,
    effective: EffectiveDate,
    retrieved_at: RetrievalInstant,
}

impl<'run> PublishableRules<'run> {
    /// The only producer, and it is `pub(crate)`.
    pub(crate) const fn new(
        document: &'run OfficialDocument,
        connector: &'run ConnectorId,
        effective: EffectiveDate,
        retrieved_at: RetrievalInstant,
    ) -> Self {
        Self {
            document,
            connector,
            effective,
            retrieved_at,
        }
    }

    /// When the rules start to apply.
    #[must_use]
    pub const fn effective(&self) -> EffectiveDate {
        self.effective
    }

    /// Who they apply to.
    #[must_use]
    pub const fn scope(&self) -> &TargetScope {
        self.document.scope()
    }
}

/// What one publication put into the claim graph.
///
/// Identifiers, dates and digests. No document text: the bytes stay behind
/// [`crate::snapshot::RawSnapshot`]'s one sealed route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedRules {
    connector: ConnectorId,
    rules: Vec<RuleId>,
    effective: EffectiveDate,
    scope: TargetScope,
    retrieved_at: RetrievalInstant,
    parser_version: ParserVersion,
}

impl PublishedRules {
    /// Which connector's document.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// Which rules were published.
    #[must_use]
    pub fn rules(&self) -> &[RuleId] {
        &self.rules
    }

    /// When they start to apply.
    #[must_use]
    pub const fn effective(&self) -> EffectiveDate {
        self.effective
    }

    /// Who they apply to.
    #[must_use]
    pub const fn scope(&self) -> &TargetScope {
        &self.scope
    }

    /// When the source was retrieved.
    #[must_use]
    pub const fn retrieved_at(&self) -> RetrievalInstant {
        self.retrieved_at
    }

    /// Which parser read it.
    #[must_use]
    pub const fn parser_version(&self) -> ParserVersion {
        self.parser_version
    }
}

/// Why a document went to the review queue instead of being published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueReason {
    /// Section 29.2's undated official document.
    UnscopedOfficialSource,
    /// `IN05`. Another official source disagrees and nobody has decided.
    UnresolvedConflict,
}

impl QueueReason {
    /// Exhaustive listing.
    pub const ALL: [Self; 2] = [Self::UnscopedOfficialSource, Self::UnresolvedConflict];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnscopedOfficialSource => crate::dating::UNSCOPED_OFFICIAL_SOURCE,
            Self::UnresolvedConflict => "UNRESOLVED_CONFLICT",
        }
    }
}

/// A document that reached the end of the pipeline without being published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewQueued {
    connector: ConnectorId,
    reason: QueueReason,
    rules: Vec<RuleId>,
    conflicts: Vec<ConflictCase>,
}

impl ReviewQueued {
    /// The only producer, and it is `pub(crate)`.
    pub(crate) const fn new(
        connector: ConnectorId,
        reason: QueueReason,
        rules: Vec<RuleId>,
        conflicts: Vec<ConflictCase>,
    ) -> Self {
        Self {
            connector,
            reason,
            rules,
            conflicts,
        }
    }

    /// Which connector's document.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// Why it is here.
    #[must_use]
    pub const fn reason(&self) -> QueueReason {
        self.reason
    }

    /// Which rules were held back.
    #[must_use]
    pub fn rules(&self) -> &[RuleId] {
        &self.rules
    }

    /// The open conflict cases, if that is why.
    #[must_use]
    pub fn conflicts(&self) -> &[ConflictCase] {
        &self.conflicts
    }
}

/// What stage nine produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Publication {
    /// Claims were published.
    Published(PublishedRules),
    /// Nothing was published; a person has to look.
    Queued(ReviewQueued),
}

impl Publication {
    /// The published rules, when anything was published.
    #[must_use]
    pub const fn published(&self) -> Option<&PublishedRules> {
        match self {
            Self::Published(rules) => Some(rules),
            Self::Queued(_) => None,
        }
    }

    /// The queued document, when nothing was.
    #[must_use]
    pub const fn queued(&self) -> Option<&ReviewQueued> {
        match self {
            Self::Queued(queued) => Some(queued),
            Self::Published(_) => None,
        }
    }
}

/// Publishes the rules of a document that states when it applies.
///
/// The argument is the whole of the rule. There is no second entry point, and
/// no parameter that turns the refusal off.
#[must_use]
pub fn publish(publishable: PublishableRules<'_>) -> PublishedRules {
    PublishedRules {
        connector: publishable.connector.clone(),
        rules: publishable
            .document
            .rules()
            .iter()
            .map(|rule| rule.id().clone())
            .collect(),
        effective: publishable.effective,
        scope: publishable.document.scope().clone(),
        retrieved_at: publishable.retrieved_at,
        parser_version: publishable.document.parser_version(),
    }
}
