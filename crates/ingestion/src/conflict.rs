//! Competing official sources: the five dimensions, and the winner that is not here.
//!
//! Section 8.4, in the paragraph that follows the numbered list of collection
//! targets: *when sources conflict, a mechanical winner is not chosen by the
//! higher or lower number. The legal hierarchy of the regulation, the issuance
//! date, the effective date, the target scope and the transitional measures are
//! compared, and a `ConflictCase` is made. A dangerous determination such as
//! graduation is left `INDETERMINATE` until it is resolved.*
//!
//! [`ConflictDimension`] is those five. [`ConflictCase::open`] records one
//! finding per dimension. There is no sixth thing it then does with them:
//! **this module contains no function from a set of findings to a source.** A
//! case is [`Resolution::Unresolved`] until a person records a decision, and
//! [`ConflictCase::disposition`] is [`AuditDisposition::Indeterminate`] for as
//! long as it is.
//!
//! # Why this module holds no number
//!
//! `no_numeric_source_winner` reads this file with comments and string literals
//! removed and refuses a numeric type, a numeric literal, and every operation
//! that turns a collection into a position or a count. The five comparisons it
//! performs are named relations computed by the modules that own the values —
//! [`crate::document::LegalAuthority::hierarchy_relation`],
//! [`crate::dating::Date::relation_to`],
//! [`crate::document::TargetScope::relation_to`] and
//! [`crate::document::TransitionalMeasures::relation_to`] — so the comparison
//! this module does is selecting a dimension, never scoring one.
//!
//! Dates are compared, because two of the five dimensions *are* dates. What the
//! rule refuses is the step after that: a number that says how many dimensions
//! favoured a side, a rank read out of a list, or a source picked because it
//! came first.

use academic_domain::{ContentDigest, engines::RuleId};

use crate::{
    dating::{DateRelation, Dating, IssuanceDate},
    document::{
        HierarchyRelation, LegalAuthority, OfficialDocument, ScopeRelation, TargetScope,
        TransitionRelation, TransitionalMeasures,
    },
    identifier::{ConnectorId, DependentId},
    manifest::DeclaredTarget,
};

/// The five things section 8.4 compares when official sources disagree.
///
/// A slice rather than a fixed-size array on purpose: an array declares its
/// length, and a length is a number this module does not hold. Nothing reads a
/// position out of this list either — it exists so the five can be enumerated
/// and checked against the specification, which is what
/// `conflict_case_dimensions` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictDimension {
    /// The legal hierarchy of the issuing authority.
    LegalHierarchy,
    /// The day each document was issued.
    IssuanceDate,
    /// The day each document takes effect.
    EffectiveDate,
    /// Who each document applies to.
    TargetScope,
    /// What each document provides for the people a change catches.
    TransitionalMeasures,
}

impl ConflictDimension {
    /// Section 8.4's order, which is the order the sentence lists them in.
    pub const ALL: &'static [Self] = &[
        Self::LegalHierarchy,
        Self::IssuanceDate,
        Self::EffectiveDate,
        Self::TargetScope,
        Self::TransitionalMeasures,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegalHierarchy => "LEGAL_HIERARCHY",
            Self::IssuanceDate => "ISSUANCE_DATE",
            Self::EffectiveDate => "EFFECTIVE_DATE",
            Self::TargetScope => "TARGET_SCOPE",
            Self::TransitionalMeasures => "TRANSITIONAL_MEASURES",
        }
    }
}

/// Which of the two contending documents.
///
/// Two names, not an index. A caller that has a `Side` knows which document it
/// is talking about and cannot arrive at one by counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The document passed first to [`ConflictCase::open`].
    Left,
    /// The document passed second.
    Right,
}

impl Side {
    /// Exhaustive listing.
    pub const ALL: &'static [Self] = &[Self::Left, Self::Right];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
        }
    }
}

/// How two dates stand, when either of them may be absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateComparison {
    /// Both dates are stated and stand this way.
    Stated(DateRelation),
    /// The left document states no date.
    LeftAbsent,
    /// The right document states no date.
    RightAbsent,
    /// Neither states one.
    BothAbsent,
}

impl DateComparison {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stated(relation) => relation.as_str(),
            Self::LeftAbsent => "LEFT_ABSENT",
            Self::RightAbsent => "RIGHT_ABSENT",
            Self::BothAbsent => "BOTH_ABSENT",
        }
    }
}

/// What one dimension found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionOutcome {
    /// The legal hierarchy relation.
    Hierarchy(HierarchyRelation),
    /// How the issuance dates stand.
    Issuance(DateComparison),
    /// How the effective dates stand.
    Effective(DateComparison),
    /// How the target scopes stand.
    Scope(ScopeRelation),
    /// How the transitional measures stand.
    Transition(TransitionRelation),
}

impl DimensionOutcome {
    /// Stable spelling of the relation this outcome carries.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hierarchy(relation) => relation.as_str(),
            Self::Issuance(comparison) | Self::Effective(comparison) => comparison.as_str(),
            Self::Scope(relation) => relation.as_str(),
            Self::Transition(relation) => relation.as_str(),
        }
    }
}

/// One dimension and what comparing it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DimensionFinding {
    dimension: ConflictDimension,
    outcome: DimensionOutcome,
}

impl DimensionFinding {
    /// Which dimension.
    #[must_use]
    pub const fn dimension(&self) -> ConflictDimension {
        self.dimension
    }

    /// What it found.
    #[must_use]
    pub const fn outcome(&self) -> DimensionOutcome {
        self.outcome
    }
}

/// One document's side of a disagreement.
///
/// Everything a dimension needs, and the digest of the rule text that differs.
/// The text is not here: a conflict case carries identifiers and digests, so
/// presenting one to a person moves no document bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContendingSource {
    connector: ConnectorId,
    target: DeclaredTarget,
    authority: LegalAuthority,
    issued: Option<IssuanceDate>,
    dating: Dating,
    scope: TargetScope,
    transition: TransitionalMeasures,
    rule: RuleId,
    text_digest: ContentDigest,
}

impl ContendingSource {
    /// One side of a disagreement, read off the document it came from.
    ///
    /// The document is the argument rather than seven of its fields, so a
    /// contender cannot be assembled with an authority from one reading and a
    /// scope from another. `None` when the document carries no such rule.
    #[must_use]
    pub fn from_document(
        connector: ConnectorId,
        target: DeclaredTarget,
        document: &OfficialDocument,
        rule: &RuleId,
    ) -> Option<Self> {
        let parsed = document.rule(rule)?;
        Some(Self {
            connector,
            target,
            authority: document.authority(),
            issued: document.issued(),
            dating: document.dating(),
            scope: document.scope().clone(),
            transition: document.transitional_measures(),
            rule: parsed.id().clone(),
            text_digest: *parsed.text_digest(),
        })
    }

    /// Which connector collected it.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// Which declared document.
    #[must_use]
    pub const fn target(&self) -> DeclaredTarget {
        self.target
    }

    /// Which body issued it.
    #[must_use]
    pub const fn authority(&self) -> LegalAuthority {
        self.authority
    }

    /// When it was issued, if it says.
    #[must_use]
    pub const fn issued(&self) -> Option<IssuanceDate> {
        self.issued
    }

    /// When it takes effect, or `UNSCOPED_OFFICIAL_SOURCE`.
    #[must_use]
    pub const fn dating(&self) -> Dating {
        self.dating
    }

    /// Who it applies to.
    #[must_use]
    pub const fn scope(&self) -> &TargetScope {
        &self.scope
    }

    /// What it provides for the people a change catches.
    #[must_use]
    pub const fn transitional_measures(&self) -> TransitionalMeasures {
        self.transition
    }

    /// Which rule is in dispute.
    #[must_use]
    pub const fn rule(&self) -> &RuleId {
        &self.rule
    }

    /// The digest of this document's text for that rule.
    #[must_use]
    pub const fn text_digest(&self) -> &ContentDigest {
        &self.text_digest
    }
}

/// What a person decided about one case.
///
/// This crate records the decision; it does not decide who may make one.
/// `P2-M4` is where a non-delegable action refuses a model actor, and nothing
/// here duplicates that check or claims to have made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserResolution {
    chose: Side,
    actor: DependentId,
}

impl UserResolution {
    /// Records that a person chose one side.
    #[must_use]
    pub const fn recorded(chose: Side, actor: DependentId) -> Self {
        Self { chose, actor }
    }

    /// Which side was chosen.
    #[must_use]
    pub const fn chose(&self) -> Side {
        self.chose
    }

    /// Who chose it.
    #[must_use]
    pub const fn actor(&self) -> &DependentId {
        &self.actor
    }
}

/// Whether a case has been decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Nobody has decided. The state a case opens in and stays in.
    Unresolved,
    /// A person decided.
    ByUser(UserResolution),
}

/// What a dependent audit may conclude while a case stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditDisposition {
    /// The audit may conclude.
    Determinate,
    /// `IN05`. The audit may not conclude: two official sources disagree and
    /// nobody has decided between them.
    Indeterminate,
}

impl AuditDisposition {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Determinate => "DETERMINATE",
            Self::Indeterminate => "INDETERMINATE",
        }
    }
}

/// Two official sources that disagree about one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictCase {
    left: ContendingSource,
    right: ContendingSource,
    findings: Vec<DimensionFinding>,
    resolution: Resolution,
}

impl ConflictCase {
    /// Opens a case and records one finding per dimension.
    ///
    /// The findings are recorded in [`ConflictDimension::ALL`]'s order because
    /// that is the specification's order and a reader comparing the two should
    /// not have to sort. Nothing reads that order back as authority.
    #[must_use]
    pub fn open(left: ContendingSource, right: ContendingSource) -> Self {
        let mut findings = Vec::new();
        for dimension in ConflictDimension::ALL {
            let outcome = match dimension {
                ConflictDimension::LegalHierarchy => DimensionOutcome::Hierarchy(
                    left.authority().hierarchy_relation(right.authority()),
                ),
                ConflictDimension::IssuanceDate => DimensionOutcome::Issuance(
                    compare_optional_dates(left.issued(), right.issued()),
                ),
                ConflictDimension::EffectiveDate => {
                    DimensionOutcome::Effective(compare_optional_dates(
                        left.dating().effective_date(),
                        right.dating().effective_date(),
                    ))
                }
                ConflictDimension::TargetScope => {
                    DimensionOutcome::Scope(left.scope().relation_to(right.scope()))
                }
                ConflictDimension::TransitionalMeasures => DimensionOutcome::Transition(
                    left.transitional_measures()
                        .relation_to(right.transitional_measures()),
                ),
            };
            findings.push(DimensionFinding {
                dimension: *dimension,
                outcome,
            });
        }
        Self {
            left,
            right,
            findings,
            resolution: Resolution::Unresolved,
        }
    }

    /// The document passed first.
    #[must_use]
    pub const fn left(&self) -> &ContendingSource {
        &self.left
    }

    /// The document passed second.
    #[must_use]
    pub const fn right(&self) -> &ContendingSource {
        &self.right
    }

    /// One finding per dimension.
    #[must_use]
    pub fn findings(&self) -> &[DimensionFinding] {
        &self.findings
    }

    /// The finding for one dimension.
    #[must_use]
    pub fn finding(&self, dimension: ConflictDimension) -> Option<&DimensionFinding> {
        self.findings
            .iter()
            .find(|finding| finding.dimension() == dimension)
    }

    /// Whether anyone has decided.
    #[must_use]
    pub const fn resolution(&self) -> &Resolution {
        &self.resolution
    }

    /// Records a person's decision.
    pub fn resolve(&mut self, resolution: UserResolution) {
        self.resolution = Resolution::ByUser(resolution);
    }

    /// What a dependent audit may conclude.
    #[must_use]
    pub const fn disposition(&self) -> AuditDisposition {
        match self.resolution {
            Resolution::Unresolved => AuditDisposition::Indeterminate,
            Resolution::ByUser(_) => AuditDisposition::Determinate,
        }
    }
}

/// Opens a case when two sources really disagree, and otherwise does not.
///
/// They disagree when they speak about the same rule, their scopes are not
/// disjoint, and their text differs. Two documents that say the same thing are
/// not a conflict, and two that apply to different cohorts are not either.
#[must_use]
pub fn detect(left: ContendingSource, right: ContendingSource) -> Option<ConflictCase> {
    let same_rule = left.rule() == right.rule();
    let overlapping = left.scope().relation_to(right.scope()) != ScopeRelation::Disjoint;
    let differing_text = left.text_digest() != right.text_digest();
    (same_rule && overlapping && differing_text).then(|| ConflictCase::open(left, right))
}

/// Compares two dates either of which may be absent.
fn compare_optional_dates<T: HasDate>(left: Option<T>, right: Option<T>) -> DateComparison {
    match (left, right) {
        (None, None) => DateComparison::BothAbsent,
        (None, Some(_)) => DateComparison::LeftAbsent,
        (Some(_), None) => DateComparison::RightAbsent,
        (Some(first), Some(second)) => {
            DateComparison::Stated(first.date().relation_to(second.date()))
        }
    }
}

/// A value that carries a calendar date.
///
/// Implemented for the two dated newtypes so the comparison above is written
/// once rather than twice with the types swapped.
pub trait HasDate {
    /// The date.
    fn date(&self) -> crate::dating::Date;
}

impl HasDate for IssuanceDate {
    fn date(&self) -> crate::dating::Date {
        Self::date(*self)
    }
}

impl HasDate for crate::dating::EffectiveDate {
    fn date(&self) -> crate::dating::Date {
        Self::date(*self)
    }
}
